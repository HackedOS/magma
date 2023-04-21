use std::{
    collections::{HashMap, HashSet},
    os::fd::FromRawFd,
    path::PathBuf,
    time::{Duration},
};

use smithay::{
    backend::{
        allocator::{
            gbm::{self, GbmAllocator, GbmBufferFlags, GbmDevice},
            Fourcc, dmabuf::Dmabuf,
        },
        drm::{self, DrmDevice, DrmDeviceFd, DrmNode, NodeType, compositor::DrmCompositor, DrmError},
        egl::{EGLDevice, EGLDisplay},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{
            element::{texture::{TextureBuffer, TextureRenderElement}, surface::WaylandSurfaceRenderElement, AsRenderElements},
            gles::{GlesRenderer, GlesTexture},
            multigpu::{gbm::GbmGlesBackend, GpuManager, MultiRenderer}, ImportDma,
        },
        session::{libseat::LibSeatSession, Event as SessionEvent, Session},
        udev::{self, UdevBackend, UdevEvent}, SwapBuffersError,
    },
    desktop::{space::SpaceElement, layer_map_for_output, LayerSurface},
    output::{Mode as WlMode, Output, PhysicalProperties},
    reexports::{
        calloop::{EventLoop, LoopHandle, RegistrationToken, timer::{Timer, TimeoutAction}},
        drm::{control::{crtc::{self, Handle}, ModeTypeFlags}, Device as DrmDeviceTrait, SystemError},
        input::Libinput,
        nix::fcntl::OFlag,
        wayland_server::{Display, DisplayHandle, backend::GlobalId}, wayland_protocols::wp::linux_dmabuf::zv1::server::zwp_linux_dmabuf_feedback_v1,
    },
    utils::{DeviceFd, Scale, Transform, Size}, wayland::{shell::wlr_layer::Layer, dmabuf::{DmabufGlobal, DmabufState, DmabufHandler, ImportError, DmabufFeedbackBuilder, DmabufFeedback}}, delegate_dmabuf,
};
use smithay_drm_extras::{
    drm_scanner::{self, DrmScanEvent, DrmScanner},
    edid::EdidInfo,
};
use tracing::{error, info, warn, trace};

use crate::{
    state::{Backend, CalloopData, MagmaState},
    utils::{render::CustomRenderElements},
};
pub type GbmDrmCompositor = DrmCompositor<
    GbmAllocator<DrmDeviceFd>,
    GbmDevice<DrmDeviceFd>,
    (),
    DrmDeviceFd,
>;
const SUPPORTED_FORMATS: &[Fourcc] = &[
    Fourcc::Abgr2101010,
    Fourcc::Argb2101010,
    Fourcc::Abgr8888,
    Fourcc::Argb8888,
];

static CURSOR_DATA: &[u8] = include_bytes!("../../resources/cursor.rgba");

pub struct UdevData {
    pub session: LibSeatSession,
    handle: LoopHandle<'static, CalloopData<UdevData>>,
    primary_gpu: DrmNode,
    gpus: GpuManager<GbmGlesBackend<GlesRenderer>>,
    devices: HashMap<DrmNode, Device>,
    dmabuf_state: Option<(DmabufState, DmabufGlobal)>,
}

impl DmabufHandler for MagmaState<UdevData> {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.backend_data.dmabuf_state.as_mut().unwrap().0
    }

    fn dmabuf_imported(&mut self, _global: &DmabufGlobal, dmabuf: Dmabuf) -> Result<(), ImportError> {
        self.backend_data
            .gpus
            .single_renderer(&self.backend_data.primary_gpu)
            .and_then(|mut renderer| renderer.import_dmabuf(&dmabuf, None))
            .map(|_| ())
            .map_err(|_| ImportError::Failed)
    }
}
delegate_dmabuf!(MagmaState<UdevData>);

impl Backend for UdevData {
    fn seat_name(&self) -> String {
        self.session.seat()
    }
}
pub struct Device {
    pub surfaces: HashMap<crtc::Handle, Surface>,
    pub gbm: GbmDevice<DrmDeviceFd>,
    pub drm: DrmDevice,
    pub drm_scanner: DrmScanner,
    pub render_node: DrmNode,
    pub registration_token: RegistrationToken,
}

pub fn init_udev() {
    let mut event_loop: EventLoop<CalloopData<UdevData>> = EventLoop::try_new().unwrap();
    let mut display: Display<MagmaState<UdevData>> = Display::new().unwrap();

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
        dmabuf_state: None,
    };

    let mut state = MagmaState::new(event_loop.handle(), event_loop.get_signal(), &mut display, data);
    /*
     * Add input source
     */
    let mut libinput_context = Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(
        state.backend_data.session.clone().into(),
    );
    libinput_context
        .udev_assign_seat(&state.backend_data.session.seat())
        .unwrap();

    let libinput_backend = LibinputInputBackend::new(libinput_context.clone());

    event_loop
        .handle()
        .insert_source(libinput_backend, move |event, _, calloopdata| {
            if let Some(vt) = calloopdata.state.process_input_event_udev(event) {
                info!(to = vt, "Trying to switch vt");
                if let Err(err) = calloopdata.state.backend_data.session.change_vt(vt) {
                    error!(vt, "Error switching vt: {}", err);
                }
            }
        })
        .unwrap();

        event_loop
        .handle()
        .insert_source(notifier, move |event, _, data| {
            match event {
                SessionEvent::PauseSession => {
                    libinput_context.suspend();
                    info!("pausing session");
    
                    for backend in data.state.backend_data.devices.values() {
                        backend.drm.pause();
                    }
                }
                SessionEvent::ActivateSession => {
                    info!("resuming session");
    
                    if let Err(err) = libinput_context.resume() {
                        error!("Failed to resume libinput context: {:?}", err);
                    }
                    for (node, backend) in data
                        .state
                        .backend_data
                        .devices
                        .iter_mut()
                        .map(|(handle, backend)| (*handle, backend))
                    {
                        backend.drm.activate();
                        for (crtc, surface) in backend.surfaces.iter_mut().map(|(handle, surface)| (*handle, surface)) {
                            if let Err(err) = surface.compositor.surface().reset_state() {
                                warn!("Failed to reset drm surface state: {}", err);
                            }
                            // reset the buffers after resume to trigger a full redraw
                            // this is important after a vt switch as the primary plane
                            // has no content and damage tracking may prevent a redraw
                            // otherwise
                            surface.compositor.reset_buffers();
                            data.state.loop_handle.insert_idle(move |data| data.state.render(node, crtc));

                        }
                    }
                }
            }
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

    let renderer = state.backend_data.gpus.single_renderer(&primary_gpu).unwrap();
    // init dmabuf support with format list from our primary gpu
    let dmabuf_formats = renderer.dmabuf_formats().collect::<Vec<_>>();
    let default_feedback = DmabufFeedbackBuilder::new(primary_gpu.dev_id(), dmabuf_formats)
        .build()
        .unwrap();
    let mut dmabuf_state = DmabufState::new();
    let global = dmabuf_state
        .create_global_with_default_feedback::<MagmaState<UdevData>>(&display.handle(), &default_feedback);
    state.backend_data.dmabuf_state = Some((dmabuf_state, global));

    let gpus = &mut state.backend_data.gpus;
    state.backend_data.devices.values_mut().for_each(|backend_data| {
        // Update the per drm surface dmabuf feedback
        backend_data.surfaces.values_mut().for_each(|surface_data| {
            surface_data.dmabuf_feedback = surface_data.dmabuf_feedback.take().or_else(|| {
                get_surface_dmabuf_feedback(
                    primary_gpu,
                    surface_data.render_node,
                    gpus,
                    &surface_data.compositor,
                )
            });
        });
    });

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

    event_loop
        .run(None, &mut calloopdata, move |data| {
            data.state.workspaces.all_windows().for_each(|e| e.refresh());
            data.state.popup_manager.cleanup();
            data.display.flush_clients().unwrap();
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
impl MagmaState<UdevData> {
    pub fn on_drm_event(
        &mut self,
        node: DrmNode,
        event: drm::DrmEvent,
        _meta: &mut Option<drm::DrmEventMetadata>,
    ) {
        match event {
            drm::DrmEvent::VBlank(crtc) => {
                let device = self.backend_data.devices.get_mut(&node).unwrap();
                let surface = device.surfaces.get_mut(&crtc).unwrap();
                surface.compositor.frame_submitted().unwrap();
                self.render(
                    node,
                    crtc,
                );
                
            }
            drm::DrmEvent::Error(_) => {}
        }
    }

    pub fn on_connector_event(
        &mut self,
        node: DrmNode,
        event: drm_scanner::DrmScanEvent,
        display: &mut Display<MagmaState<UdevData>>,
    ) {
        let device = if let Some(device) = self.backend_data.devices.get_mut(&node) {
            device
        } else {
            error!("Received connector event for unknown device: {:?}", node);
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

                let name = format!(
                    "{}-{}",
                    connector.interface().as_str(),
                    connector.interface_id()
                );
        
                let drm_mode = if self.config.outputs.contains_key(&name) {
                    let output_config = &self.config.outputs[&name];
                    *connector
                        .modes()
                        .iter()
                        .filter(|mode| {
                            let (x, y) = mode.size();
                            Size::from((x as i32, y as i32)) == output_config.mode_size()
                        })
                            // and then select the closest refresh rate (e.g. to match 59.98 as 60)
                        .min_by_key(|mode| {
                            let refresh_rate = WlMode::from(**mode).refresh;
                            (output_config.mode_refresh() as i32 - refresh_rate as i32).abs()
                        }).expect("No matching mode found for output config")
                } else {
                    *connector
                    .modes()
                    .iter()
                    .find(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
                    .unwrap_or(&connector.modes()[0])
                };
        
                let drm_surface = device.drm
                    .create_surface(crtc, drm_mode, &[connector.handle()])
                    .unwrap();
        
        
                let (make, model) = EdidInfo::for_connector(&device.drm, connector.handle())
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
                let global = output.create_global::<MagmaState<UdevData>>(&display.handle());
                let output_mode = WlMode::from(drm_mode);
                output.set_preferred(output_mode);
                output.change_current_state(
                    Some(output_mode),
                    Some(Transform::Normal),
                    Some(smithay::output::Scale::Integer(1)),
                    None,
                );
                let render_formats = renderer.as_mut().egl_context().dmabuf_render_formats().clone();
                let gbm_allocator = GbmAllocator::new(device.gbm.clone(), GbmBufferFlags::RENDERING);
                
                let driver = match device.drm.get_driver() {
                    Ok(driver) => driver,
                    Err(err) => {
                        warn!("Failed to query drm driver: {}", err);
                        return;
                    }
                };
    
                let mut planes = match drm_surface.planes() {
                    Ok(planes) => planes,
                    Err(err) => {
                        warn!("Failed to query surface planes: {}", err);
                        return;
                    }
                };
    
                // Using an overlay plane on a nvidia card breaks
                if driver.name().to_string_lossy().to_lowercase().contains("nvidia")
                    || driver
                        .description()
                        .to_string_lossy()
                        .to_lowercase()
                        .contains("nvidia")
                {
                    planes.overlay = vec![];
                }

                let compositor = GbmDrmCompositor::new(
                    &output,
                    drm_surface,
                    Some(planes),
                    gbm_allocator,
                    device.gbm.clone(),
                    SUPPORTED_FORMATS,
                    render_formats,
                    device.drm.cursor_size(),
                    Some(device.gbm.clone())
        
                ).unwrap();

                let dmabuf_feedback = get_surface_dmabuf_feedback(
                    self.backend_data.primary_gpu,
                    device.render_node,
                    &mut self.backend_data.gpus,
                    &compositor,
                );

                let surface = Surface {
                    _dh: display.handle(),
                    _device_id: node,
                    render_node: device.render_node,
                    _global: Some(global),
                    compositor,
                    dmabuf_feedback,
                };

                for workspace in self.workspaces.iter() {
                    workspace.add_output(output.clone())
                }

                device.surfaces.insert(crtc, surface);

                self.render(
                    node,
                    crtc,
                );
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
impl MagmaState<UdevData> {
    pub fn on_udev_event(&mut self, event: UdevEvent, display: &mut Display<MagmaState<UdevData>>) {
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
        display: &mut Display<MagmaState<UdevData>>,
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

        let registration_token = self.backend_data
            .handle
            .insert_source(drm_notifier, move |event, meta, calloopdata| {
                calloopdata
                    .state
                    .on_drm_event(node, event, meta);
            })
            .unwrap();

        self.backend_data.devices.insert(
            node,
            Device {
                drm,
                gbm,
                drm_scanner: Default::default(),
                surfaces: Default::default(),
                render_node,
                registration_token,
            },
        );

        self.on_device_changed(node, display);
    }

    fn on_device_changed(&mut self, node: DrmNode, display: &mut Display<MagmaState<UdevData>>) {
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
    _dh: DisplayHandle,
    _device_id: DrmNode,
    render_node: DrmNode,
    _global: Option<GlobalId>,
    compositor: GbmDrmCompositor,
    dmabuf_feedback: Option<DrmSurfaceDmabufFeedback>,
}

impl MagmaState<UdevData> {
    pub fn render(
        &mut self,
        node: DrmNode,
        crtc: Handle,
    ) 
    {      
        let device = self.backend_data.devices.get_mut(&node).unwrap();
        let surface = device.surfaces.get_mut(&crtc).unwrap();
        let mut renderer = self.backend_data.gpus.single_renderer(&device.render_node).unwrap();
        let output = self.workspaces.current().outputs().next().unwrap();

        let mut renderelements: Vec<CustomRenderElements<MultiRenderer<_,_>>> = vec![];

        let pointer_texture = TextureBuffer::from_memory(
            &mut renderer,
            CURSOR_DATA,
            Fourcc::Abgr8888,
            (64, 64),
            false,
            1,
            Transform::Normal,
            None,
        )
        .unwrap();

        renderelements.append(&mut vec![CustomRenderElements::<MultiRenderer<_,_>>::from(
            TextureRenderElement::from_texture_buffer(
                self.pointer_location.to_physical(Scale::from(1.0)),
                &pointer_texture,
                None,
                None,
                None,
            ),
        )]);

        let layer_map = layer_map_for_output(&output);
        let (lower, upper): (Vec<&LayerSurface>, Vec<&LayerSurface>) = layer_map
            .layers()
            .rev()
            .partition(|s| matches!(s.layer(), Layer::Background | Layer::Bottom));

        renderelements.extend(
            upper
                .into_iter()
                .filter_map(|surface| {
                    layer_map
                        .layer_geometry(surface)
                        .map(|geo| (geo.loc, surface))
                })
                .flat_map(|(loc, surface)| {
                    AsRenderElements::<MultiRenderer<_,_>>::render_elements::<WaylandSurfaceRenderElement<MultiRenderer<_,_>>>(
                        surface,
                        &mut renderer,
                        loc.to_physical_precise_round(1),
                        Scale::from(1.0),
                    )
                    .into_iter()
                    .map(CustomRenderElements::Surface)
                }),
        );

        renderelements.extend(self.workspaces.current().render_elements(&mut renderer));

        renderelements.extend(
            lower
                .into_iter()
                .filter_map(|surface| {
                    layer_map
                        .layer_geometry(surface)
                        .map(|geo| (geo.loc, surface))
                })
                .flat_map(|(loc, surface)| {
                    AsRenderElements::<MultiRenderer<_,_>>::render_elements::<WaylandSurfaceRenderElement<MultiRenderer<_,_>>>(
                        surface,
                        &mut renderer,
                        loc.to_physical_precise_round(1),
                        Scale::from(1.0),
                    )
                    .into_iter()
                    .map(CustomRenderElements::Surface)
                }),
        );

        let rendered = surface.compositor
            .render_frame::<_, _, GlesTexture>(
                &mut renderer,
                &renderelements,
                [0.1, 0.1, 0.1, 1.0],
            )
            .unwrap().damage.is_some();
        
        let mut result = Ok(rendered);
        if rendered {
            let queueresult = surface.compositor.queue_frame(()).map_err(Into::<SwapBuffersError>::into);
            if queueresult.is_err() {result = Err(queueresult.unwrap_err());}
        }

        let reschedule = match &result {
            Ok(has_rendered) => !has_rendered,
            Err(err) => {
                warn!("Error during rendering: {:?}", err);
                match err {
                    SwapBuffersError::AlreadySwapped => false,
                    SwapBuffersError::TemporaryFailure(err) => !matches!(
                        err.downcast_ref::<DrmError>(),
                        Some(&DrmError::DeviceInactive)
                            | Some(&DrmError::Access {
                                source: SystemError::PermissionDenied,
                                ..
                            })
                    ),
                    SwapBuffersError::ContextLost(err) => panic!("Rendering loop lost: {}", err),
                }
            }
        };

        if reschedule  {
            let output_refresh = match output.current_mode() {
                Some(mode) => mode.refresh,
                None => return,
            };
            // If reschedule is true we either hit a temporary failure or more likely rendering
            // did not cause any damage on the output. In this case we just re-schedule a repaint
            // after approx. one frame to re-test for damage.
            let reschedule_duration = Duration::from_millis((1_000_000f32 / output_refresh as f32) as u64);
            trace!(
                "reschedule repaint timer with delay {:?} on {:?}",
                reschedule_duration,
                crtc,
            );
            let timer = Timer::from_duration(reschedule_duration);
            self.loop_handle
                .insert_source(timer, move |_, _, data| {
                    data.state.render(node, crtc);
                    TimeoutAction::Drop
                })
                .expect("failed to schedule frame timer");
        }

        self.workspaces.current().windows().for_each(|window| {
            window.send_frame(
                &output,
                self.start_time.elapsed(),
                Some(Duration::ZERO),
                |_, _| Some(output.clone()),
            );
        });
        for layer_surface in layer_map.layers() {
            layer_surface.send_frame(
                &output,
                self.start_time.elapsed(),
                Some(Duration::ZERO),
                |_, _| Some(output.clone()),
            );
        }
    }
}

fn get_surface_dmabuf_feedback(
    primary_gpu: DrmNode,
    render_node: DrmNode,
    gpus: &mut GpuManager<GbmGlesBackend<GlesRenderer>>,
    compositor: &GbmDrmCompositor,
) -> Option<DrmSurfaceDmabufFeedback> {
    let primary_formats = gpus
        .single_renderer(&primary_gpu)
        .ok()?
        .dmabuf_formats()
        .collect::<HashSet<_>>();

    let render_formats = gpus
        .single_renderer(&render_node)
        .ok()?
        .dmabuf_formats()
        .collect::<HashSet<_>>();

    let all_render_formats = primary_formats
        .iter()
        .chain(render_formats.iter())
        .copied()
        .collect::<HashSet<_>>();

    let surface = compositor.surface();
    let planes = surface.planes().unwrap();
    // We limit the scan-out trache to formats we can also render from
    // so that there is always a fallback render path available in case
    // the supplied buffer can not be scanned out directly
    let planes_formats = surface
        .supported_formats(planes.primary.handle)
        .unwrap()
        .into_iter()
        .chain(
            planes
                .overlay
                .iter()
                .flat_map(|p| surface.supported_formats(p.handle).unwrap()),
        )
        .collect::<HashSet<_>>()
        .intersection(&all_render_formats)
        .copied()
        .collect::<Vec<_>>();

    let builder = DmabufFeedbackBuilder::new(primary_gpu.dev_id(), primary_formats);
    let render_feedback = builder
        .clone()
        .add_preference_tranche(render_node.dev_id(), None, render_formats.clone())
        .build()
        .unwrap();

    let scanout_feedback = builder
        .add_preference_tranche(
            surface.device_fd().dev_id().unwrap(),
            Some(zwp_linux_dmabuf_feedback_v1::TrancheFlags::Scanout),
            planes_formats,
        )
        .add_preference_tranche(render_node.dev_id(), None, render_formats)
        .build()
        .unwrap();

    Some(DrmSurfaceDmabufFeedback {
        render_feedback,
        scanout_feedback,
    })
}

struct DrmSurfaceDmabufFeedback {
    render_feedback: DmabufFeedback,
    scanout_feedback: DmabufFeedback,
}