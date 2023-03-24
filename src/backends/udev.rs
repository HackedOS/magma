use std::{
    collections::{HashMap, HashSet},
    os::fd::FromRawFd,
    path::PathBuf,
};

use smithay::{
    backend::{
        allocator::{
            dmabuf::{Dmabuf, DmabufAllocator},
            gbm::{self, GbmAllocator, GbmBufferFlags, GbmDevice},
            Format,
        },
        drm::{self, DrmDevice, DrmDeviceFd, DrmNode, GbmBufferedSurface, NodeType},
        egl::{EGLDevice, EGLDisplay},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{
            damage::DamageTrackedRenderer,
            element::memory::MemoryRenderBufferRenderElement,
            gles2::Gles2Renderer,
            multigpu::{gbm::GbmGlesBackend, GpuManager},
            Bind, ImportMem, Renderer,
        },
        session::{libseat::LibSeatSession, Session},
        udev::{self, UdevBackend, UdevEvent},
    },
    output::{Mode as WlMode, Output, PhysicalProperties},
    reexports::{
        calloop::{EventLoop, LoopHandle},
        drm::control::{connector, crtc, ModeTypeFlags},
        input::Libinput,
        nix::fcntl::OFlag,
        wayland_server::Display,
    },
    utils::{DeviceFd, Transform},
};
use smithay_drm_extras::{
    drm_scanner::{self, DrmScanEvent},
    edid::EdidInfo,
};
use tracing::{error, info};

use crate::state::{Backend, CalloopData, HoloState};

pub struct UdevData {
    pub session: LibSeatSession,
    pub handle: LoopHandle<'static, CalloopData<UdevData>>,
    pub primary_gpu: DrmNode,
    pub gpus: GpuManager<GbmGlesBackend<Gles2Renderer>>,
    pub devices: HashMap<DrmNode, Device>,
}

impl Backend for UdevData {
    fn seat_name(&self) -> String {
        self.session.seat()
    }
}
pub struct Device {
    pub surfaces: HashMap<crtc::Handle, Surface>,
    pub gbm: GbmDevice<DrmDeviceFd>,
    pub drm: DrmDevice,
    pub render_node: DrmNode,
    pub drm_scanner: drm_scanner::DrmScanner,
    pub gbm_allocator: DmabufAllocator<GbmAllocator<DrmDeviceFd>>,
}

pub fn init_udev() {
    let mut event_loop: EventLoop<CalloopData<UdevData>> = EventLoop::try_new().unwrap();
    let mut display: Display<HoloState<UdevData>> = Display::new().unwrap();

    /*
     * Initialize session
     */
    let (session, notifier) = match LibSeatSession::new() {
        Ok(ret) => ret,
        Err(err) => {
            error!("Could not initialize a session: {}", err);
            return;
        }
    };
    event_loop
        .handle()
        .insert_source(notifier, |_, _, _| {})
        .unwrap();

    /*
     * Initialize the compositor
     */
    let (primary_gpu, _) = primary_gpu(&session.seat());
    info!("Using {} as primary gpu.", primary_gpu);

    let gpus = GpuManager::new(Default::default()).unwrap();

    let data = UdevData {
        handle: event_loop.handle(),
        session,
        primary_gpu,
        gpus,
        devices: HashMap::new(),
    };

    let mut state = HoloState::new(&mut event_loop, &mut display, data);
    /*
     * Add input source
     */
    let mut libinput_context = Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(
        state.backend_data.session.clone().into(),
    );
    libinput_context
        .udev_assign_seat(&state.backend_data.session.seat())
        .unwrap();

    let libinput_backend = LibinputInputBackend::new(libinput_context);

    event_loop
        .handle()
        .insert_source(libinput_backend, move |event, _, calloopdata| {
            calloopdata.state.process_input_event(event)
        })
        .unwrap();

    /*
     * Initialize Udev
     */

    let backend = UdevBackend::new(&state.seat_name).unwrap();
    for (device_id, path) in backend.device_list() {
        state.backend_data.on_udev_event(UdevEvent::Added {
            device_id,
            path: path.to_owned(),
        });
    }

    event_loop
        .handle()
        .insert_source(backend, |event, _, calloopdata| {
            calloopdata.state.backend_data.on_udev_event(event)
        })
        .unwrap();

    let mut calloopdata = CalloopData { state, display };

    event_loop
        .run(None, &mut calloopdata, move |_| {
            // HoloWM is running
        })
        .unwrap();
}

pub fn primary_gpu(seat: &str) -> (DrmNode, PathBuf) {
    // TODO: can't this be in smithay?
    // primary_gpu() does the same thing anyway just without `NodeType::Render` check
    // so perhaps `primary_gpu(seat, node_type)`?
    udev::primary_gpu(seat)
        .unwrap()
        .and_then(|p| {
            DrmNode::from_path(&p)
                .ok()?
                .node_with_type(NodeType::Render)?
                .ok()
                .map(|node| (node, p))
        })
        .unwrap_or_else(|| {
            udev::all_gpus(seat)
                .unwrap()
                .into_iter()
                .find_map(|p| {
                    DrmNode::from_path(&p)
                        .ok()?
                        .node_with_type(NodeType::Render)?
                        .ok()
                        .map(|node| (node, p))
                })
                .expect("No GPU!")
        })
}

// Drm
impl UdevData {
    pub fn on_drm_event(
        &mut self,
        node: DrmNode,
        event: drm::DrmEvent,
        _meta: &mut Option<drm::DrmEventMetadata>,
    ) {
        match event {
            drm::DrmEvent::VBlank(crtc) => {
                if let Some(device) = self.devices.get_mut(&node) {
                    if let Some(surface) = device.surfaces.get_mut(&crtc) {
                        let mut renderer = if self.primary_gpu == device.render_node {
                            self.gpus.single_renderer(&device.render_node).unwrap()
                        } else {
                            self.gpus
                                .renderer(
                                    &self.primary_gpu,
                                    &device.render_node,
                                    &mut device.gbm_allocator,
                                    surface.gbm_surface.format(),
                                )
                                .unwrap()
                        };

                        surface.gbm_surface.frame_submitted().unwrap();
                        surface.next_buffer(&mut renderer);
                    }
                }
            }
            drm::DrmEvent::Error(_) => {}
        }
    }

    pub fn on_connector_event(&mut self, node: DrmNode, event: drm_scanner::DrmScanEvent) {
        let device = if let Some(device) = self.devices.get_mut(&node) {
            device
        } else {
            return;
        };

        match event {
            DrmScanEvent::Connected {
                connector,
                crtc: Some(crtc),
            } => {
                let mut renderer = self.gpus.single_renderer(&device.render_node).unwrap();

                let mut surface = Surface::new(
                    crtc,
                    &connector,
                    renderer
                        .as_mut()
                        .egl_context()
                        .dmabuf_render_formats()
                        .clone(),
                    &device.drm,
                    device.gbm.clone(),
                );

                surface.next_buffer(renderer.as_mut());

                device.surfaces.insert(crtc, surface);
            }
            DrmScanEvent::Disconnected {
                crtc: Some(crtc), ..
            } => {
                device.surfaces.remove(&crtc);
            }
            _ => {}
        }
    }
}

// Udev
impl UdevData {
    pub fn on_udev_event(&mut self, event: UdevEvent) {
        match event {
            UdevEvent::Added { device_id, path } => {
                if let Ok(node) = DrmNode::from_dev_id(device_id) {
                    self.on_device_added(node, path);
                }
            }
            UdevEvent::Changed { device_id } => {
                if let Ok(node) = DrmNode::from_dev_id(device_id) {
                    self.on_device_changed(node);
                }
            }
            UdevEvent::Removed { device_id } => {
                if let Ok(node) = DrmNode::from_dev_id(device_id) {
                    self.on_device_removed(node);
                }
            }
        }
    }

    fn on_device_added(&mut self, node: DrmNode, path: PathBuf) {
        let fd = self
            .session
            .open(
                &path,
                OFlag::O_RDWR | OFlag::O_CLOEXEC | OFlag::O_NOCTTY | OFlag::O_NONBLOCK,
            )
            .unwrap();

        let fd = DrmDeviceFd::new(unsafe { DeviceFd::from_raw_fd(fd) });

        let (drm, drm_notifier) = drm::DrmDevice::new(fd, false).unwrap();

        let gbm = gbm::GbmDevice::new(drm.device_fd().clone()).unwrap();
        let gbm_allocator = GbmAllocator::new(gbm.clone(), GbmBufferFlags::RENDERING);

        // Make sure display is dropped before we call add_node
        let render_node =
            match EGLDevice::device_for_display(&EGLDisplay::new(gbm.clone()).unwrap())
                .ok()
                .and_then(|x| x.try_get_render_node().ok().flatten())
            {
                Some(node) => node,
                None => node,
            };

        self.gpus
            .as_mut()
            .add_node(render_node, gbm.clone())
            .unwrap();

        self.handle
            .insert_source(drm_notifier, move |event, meta, calloopdata| {
                calloopdata
                    .state
                    .backend_data
                    .on_drm_event(node, event, meta)
            })
            .unwrap();

        self.devices.insert(
            node,
            Device {
                drm,
                gbm,
                gbm_allocator: DmabufAllocator(gbm_allocator),
                drm_scanner: Default::default(),
                surfaces: Default::default(),
                render_node,
            },
        );

        self.on_device_changed(node);
    }

    fn on_device_changed(&mut self, node: DrmNode) {
        if let Some(device) = self.devices.get_mut(&node) {
            for event in device.drm_scanner.scan_connectors(&device.drm) {
                self.on_connector_event(node, event);
            }
        }
    }

    fn on_device_removed(&mut self, node: DrmNode) {
        if let Some(device) = self.devices.get_mut(&node) {
            self.gpus.as_mut().remove_node(&device.render_node);
        }
    }
}

pub struct Surface {
    pub gbm_surface: GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, ()>,
    pub output: Output,
    pub damage_tracked_renderer: DamageTrackedRenderer,
}

impl Surface {
    pub fn new(
        crtc: crtc::Handle,
        connector: &connector::Info,
        formats: HashSet<Format>,
        drm: &drm::DrmDevice,
        gbm: gbm::GbmDevice<DrmDeviceFd>,
    ) -> Self {
        let mode_id = connector
            .modes()
            .iter()
            .position(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
            .unwrap_or(0);

        let drm_mode = connector.modes()[mode_id];

        let drm_surface = drm
            .create_surface(crtc, drm_mode, &[connector.handle()])
            .unwrap();

        let gbm_surface = GbmBufferedSurface::new(
            drm_surface,
            GbmAllocator::new(gbm, GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT),
            formats,
        )
        .unwrap();

        let name = smithay_drm_extras::format_connector_name(connector);

        let (make, model) = EdidInfo::for_connector(drm, connector.handle())
            .map(|info| (info.manufacturer, info.model))
            .unwrap_or_else(|| ("Unknown".into(), "Unknown".into()));

        let (w, h) = connector.size().unwrap_or((0, 0));
        let output = Output::new(
            name,
            PhysicalProperties {
                size: (w as i32, h as i32).into(),
                subpixel: smithay::output::Subpixel::Unknown,
                make,
                model,
            },
        );

        let output_mode = WlMode::from(drm_mode);
        output.set_preferred(output_mode);
        output.change_current_state(
            Some(output_mode),
            Some(Transform::Normal),
            Some(smithay::output::Scale::Integer(1)),
            None,
        );

        let damage_tracked_renderer = DamageTrackedRenderer::from_output(&output);

        Self {
            gbm_surface,
            output,
            damage_tracked_renderer,
        }
    }

    pub fn next_buffer<R>(&mut self, renderer: &mut R)
    where
        R: Renderer + ImportMem + Bind<Dmabuf>,
        R::TextureId: 'static,
    {
        let (dmabuf, age) = self.gbm_surface.next_buffer().unwrap();
        renderer.bind(dmabuf).unwrap();

        self.damage_tracked_renderer
            .render_output::<MemoryRenderBufferRenderElement<R>, _>(
                renderer,
                age as usize,
                &[],
                [1.0, 0.0, 0.0, 1.0],
            )
            .unwrap();

        self.gbm_surface.queue_buffer(None, ()).unwrap();
    }
}
