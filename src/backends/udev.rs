use std::{collections::HashMap, path::PathBuf};

use smithay::{
    backend::{
        allocator::{
            dmabuf::{AnyError, Dmabuf, DmabufAllocator},
            gbm::{GbmAllocator, GbmDevice},
            Allocator,
        },
        drm::{DrmDevice, DrmDeviceFd, DrmNode, NodeType},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{
            gles2::Gles2Renderer,
            multigpu::{gbm::GbmGlesBackend, GpuManager},
        },
        session::{libseat::LibSeatSession, Session},
        udev::{self, all_gpus, UdevBackend, UdevEvent},
    },
    reexports::{
        calloop::{EventLoop, LoopHandle, RegistrationToken},
        drm::control::crtc,
        input::Libinput,
        wayland_server::{Display, DisplayHandle},
    },
    wayland::dmabuf::{DmabufGlobal, DmabufState},
};
use smithay_drm_extras::drm_scanner;
use tracing::{error, info};

use crate::{
    state::{Backend, CalloopData, HoloState},
    surface::Surface,
};

pub struct UdevData {
    pub session: LibSeatSession,
    pub handle: LoopHandle<'static, CalloopData<UdevData>>,
    dh: DisplayHandle,
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

    /*
     * Initialize the compositor
     */
    let (primary_gpu, _) = primary_gpu(&session.seat());
    info!("Using {} as primary gpu.", primary_gpu);

    let gpus = GpuManager::new(GbmGlesBackend::default()).unwrap();

    let data = UdevData {
        handle: event_loop.handle(),
        dh: display.handle(),
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
