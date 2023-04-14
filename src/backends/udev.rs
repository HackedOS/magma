use std::{
    collections::{HashMap, HashSet},
    os::fd::FromRawFd,
    path::PathBuf,
    time::{Duration, Instant},
};

use smithay::{
    backend::{
        allocator::{
            dmabuf::{Dmabuf, DmabufAllocator},
            gbm::{self, GbmAllocator, GbmBufferFlags, GbmDevice},
            Format, Fourcc,
        },
        drm::{self, DrmDevice, DrmDeviceFd, DrmNode, GbmBufferedSurface, NodeType},
        egl::{EGLDevice, EGLDisplay},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{
            damage::OutputDamageTracker,
            element::surface::WaylandSurfaceRenderElement,
            gles2::Gles2Renderer,
            multigpu::{gbm::GbmGlesBackend, GpuManager},
            Bind, ImportAll, ImportMem, Renderer,
        },
        session::{libseat::LibSeatSession, Session},
        udev::{self, UdevBackend, UdevEvent},
    },
    desktop::space::SpaceElement,
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

use crate::{
    state::{Backend, CalloopData, HoloState},
    utils::workspaces::Workspaces,
};

const SUPPORTED_FORMATS: &[Fourcc] = &[
    Fourcc::Abgr2101010,
    Fourcc::Argb2101010,
    Fourcc::Abgr8888,
    Fourcc::Argb8888,
];
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
        state.on_udev_event(
            UdevEvent::Added {
                device_id,
                path: path.to_owned(),
            },
            &mut display,
        );
    }

    event_loop
        .handle()
        .insert_source(backend, |event, _, calloopdata| {
            calloopdata
                .state
                .on_udev_event(event, &mut calloopdata.display)
        })
        .unwrap();

    let mut calloopdata = CalloopData { state, display };

    std::env::set_var("WAYLAND_DISPLAY", &calloopdata.state.socket_name);

    std::process::Command::new("alacritty").spawn().ok();

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
impl HoloState<UdevData> {
    pub fn on_drm_event(
        &mut self,
        node: DrmNode,
        event: drm::DrmEvent,
        _meta: &mut Option<drm::DrmEventMetadata>,
        display: &mut Display<HoloState<UdevData>>,
    ) {
        match event {
            drm::DrmEvent::VBlank(crtc) => {
                if let Some(device) = self.backend_data.devices.get_mut(&node) {
                    if let Some(surface) = device.surfaces.get_mut(&crtc) {
                        let mut renderer = if self.backend_data.primary_gpu == device.render_node {
                            self.backend_data
                                .gpus
                                .single_renderer(&device.render_node)
                                .unwrap()
                        } else {
                            self.backend_data
                                .gpus
                                .renderer(
                                    &self.backend_data.primary_gpu,
                                    &device.render_node,
                                    &mut device.gbm_allocator,
                                    surface.gbm_surface.format(),
                                )
                                .unwrap()
                        };

                        surface.gbm_surface.frame_submitted().unwrap();
                        surface.next_buffer(&mut renderer, &mut self.workspaces, display);
                    }
                }
            }
            drm::DrmEvent::Error(_) => {}
        }
    }

    pub fn on_connector_event(
        &mut self,
        node: DrmNode,
        event: drm_scanner::DrmScanEvent,
        display: &mut Display<HoloState<UdevData>>,
    ) {
        let device = if let Some(device) = self.backend_data.devices.get_mut(&node) {
            device
        } else {
            return;
        };

        match event {
            DrmScanEvent::Connected {
                connector,
                crtc: Some(crtc),
            } => {
                let mut renderer = self
                    .backend_data
                    .gpus
                    .single_renderer(&device.render_node)
                    .unwrap();

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

                for workspace in self.workspaces.iter() {
                    workspace.add_output(surface.output.clone())
                }

                surface.next_buffer(renderer.as_mut(), &mut self.workspaces, display);

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
impl HoloState<UdevData> {
    pub fn on_udev_event(&mut self, event: UdevEvent, display: &mut Display<HoloState<UdevData>>) {
        match event {
            UdevEvent::Added { device_id, path } => {
                if let Ok(node) = DrmNode::from_dev_id(device_id) {
                    self.on_device_added(node, path, display);
                }
            }
            UdevEvent::Changed { device_id } => {
                if let Ok(node) = DrmNode::from_dev_id(device_id) {
                    self.on_device_changed(node, display);
                }
            }
            UdevEvent::Removed { device_id } => {
                if let Ok(node) = DrmNode::from_dev_id(device_id) {
                    self.on_device_removed(node);
                }
            }
        }
    }

    fn on_device_added(
        &mut self,
        node: DrmNode,
        path: PathBuf,
        display: &mut Display<HoloState<UdevData>>,
    ) {
        let fd = self
            .backend_data
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

        self.backend_data
            .gpus
            .as_mut()
            .add_node(render_node, gbm.clone())
            .unwrap();

        self.backend_data
            .handle
            .insert_source(drm_notifier, move |event, meta, calloopdata| {
                calloopdata
                    .state
                    .on_drm_event(node, event, meta, &mut calloopdata.display);
            })
            .unwrap();

        self.backend_data.devices.insert(
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

        self.on_device_changed(node, display);
    }

    fn on_device_changed(&mut self, node: DrmNode, display: &mut Display<HoloState<UdevData>>) {
        if let Some(device) = self.backend_data.devices.get_mut(&node) {
            for event in device.drm_scanner.scan_connectors(&device.drm) {
                self.on_connector_event(node, event, display);
            }
        }
    }

    fn on_device_removed(&mut self, node: DrmNode) {
        if let Some(device) = self.backend_data.devices.get_mut(&node) {
            self.backend_data
                .gpus
                .as_mut()
                .remove_node(&device.render_node);
        }
    }
}

pub struct Surface {
    pub gbm_surface: GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, ()>,
    pub output: Output,
    pub output_damage_tracker: OutputDamageTracker,
    pub start_time: Instant,
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
            SUPPORTED_FORMATS,
            formats,
        )
        .unwrap();

        let name = format!(
            "{}-{}",
            connector.interface().as_str(),
            connector.interface_id()
        );

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

        let output_damage_tracker = OutputDamageTracker::from_output(&output);

        let start_time = Instant::now();

        Self {
            start_time,
            gbm_surface,
            output,
            output_damage_tracker,
        }
    }

    pub fn next_buffer<R>(
        &mut self,
        renderer: &mut R,
        workspaces: &mut Workspaces,
        display: &mut Display<HoloState<UdevData>>,
    ) where
        R: Renderer + ImportMem + Bind<Dmabuf> + ImportAll,
        R::TextureId: 'static + Clone,
    {
        let (dmabuf, age) = self.gbm_surface.next_buffer().unwrap();
        renderer.bind(dmabuf).unwrap();
        let mut renderelements: Vec<WaylandSurfaceRenderElement<_>> = vec![];

        renderelements.extend(workspaces.current().render_elements(renderer));

        self.output_damage_tracker
            .render_output::<WaylandSurfaceRenderElement<R>, _>(
                renderer,
                age as usize,
                &renderelements,
                [0.1, 0.1, 0.1, 1.0],
            )
            .unwrap();

        self.gbm_surface.queue_buffer(None, ()).unwrap();

        workspaces.current().windows().for_each(|window| {
            window.send_frame(
                &self.output,
                self.start_time.elapsed(),
                Some(Duration::ZERO),
                |_, _| Some(self.output.clone()),
            );
        });
        workspaces.all_windows().for_each(|e| e.refresh());
        display.flush_clients().unwrap();
    }
}
