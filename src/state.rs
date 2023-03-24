use std::{ffi::OsString, os::fd::AsRawFd, sync::Arc};

use smithay::{
    desktop::WindowSurfaceType,
    input::{pointer::PointerHandle, Seat, SeatState},
    reexports::{
        calloop::{generic::Generic, EventLoop, Interest, LoopSignal, Mode, PostAction},
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::wl_surface::WlSurface,
            Display,
        },
    },
    utils::{Logical, Point},
    wayland::{
        compositor::CompositorState,
        data_device::DataDeviceState,
        output::OutputManagerState,
        shell::xdg::{decoration::XdgDecorationState, XdgShellState},
        shm::ShmState,
        socket::ListeningSocketSource,
    },
};
use tracing::info;

use crate::{config::Config, utils::workspace::Workspaces};

pub struct CalloopData<BackendData: Backend + 'static> {
    pub state: HoloState<BackendData>,
    pub display: Display<HoloState<BackendData>>,
}

pub struct HoloState<BackendData: Backend + 'static> {
    pub backend_data: BackendData,

    pub config: Config,

    pub start_time: std::time::Instant,
    pub socket_name: OsString,
    pub workspaces: Workspaces,
    pub loop_signal: LoopSignal,

    //Smithay State
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub xdg_decoration_state: XdgDecorationState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub data_device_state: DataDeviceState,
    pub seat_state: SeatState<HoloState<BackendData>>,

    pub seat: Seat<HoloState<BackendData>>,

    pub seat_name: String,
}

pub trait Backend {
    fn seat_name(&self) -> String;
}

impl<BackendData: Backend> HoloState<BackendData> {
    pub fn new(
        event_loop: &mut EventLoop<CalloopData<BackendData>>,
        display: &mut Display<HoloState<BackendData>>,
        backend_data: BackendData,
    ) -> Self {
        let start_time = std::time::Instant::now();

        let config = Config::load();

        info!("Config: {:#?}", config);

        let dh = display.handle();

        //Smithay State
        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let xdg_decoration_state = XdgDecorationState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let mut seat_state = SeatState::new();

        let mut seat: Seat<Self> = seat_state.new_wl_seat(&dh, "winit");

        seat.add_keyboard(Default::default(), 200, 200).unwrap();

        seat.add_pointer();

        let socket_name = Self::init_wayland_listener(display, event_loop);
        let workspaces = Workspaces::new(config.workspaces);
        let loop_signal = event_loop.get_signal();

        let seat_name = backend_data.seat_name();

        Self {
            config,
            start_time,
            socket_name,
            workspaces,
            loop_signal,
            compositor_state,
            xdg_shell_state,
            xdg_decoration_state,
            shm_state,
            output_manager_state,
            data_device_state,
            seat_state,
            seat,
            backend_data,
            seat_name,
        }
    }

    fn init_wayland_listener(
        display: &mut Display<HoloState<BackendData>>,
        event_loop: &mut EventLoop<CalloopData<BackendData>>,
    ) -> OsString {
        // Creates a new listening socket, automatically choosing the next available `wayland` socket name.
        let listening_socket = ListeningSocketSource::new_auto().unwrap();

        // Get the name of the listening socket.
        // Clients will connect to this socket.
        let socket_name = listening_socket.socket_name().to_os_string();

        let handle = event_loop.handle();

        event_loop
            .handle()
            .insert_source(listening_socket, move |client_stream, _, state| {
                // Inside the callback, you should insert the client into the display.
                //
                // You may also associate some data with the client when inserting the client.
                state
                    .display
                    .handle()
                    .insert_client(client_stream, Arc::new(ClientState))
                    .unwrap();
            })
            .expect("Failed to init the wayland event source.");

        // You also need to add the display itself to the event loop, so that client events will be processed by wayland-server.
        handle
            .insert_source(
                Generic::new(
                    display.backend().poll_fd().as_raw_fd(),
                    Interest::READ,
                    Mode::Level,
                ),
                |_, _, state| {
                    state.display.dispatch_clients(&mut state.state).unwrap();
                    Ok(PostAction::Continue)
                },
            )
            .unwrap();

        socket_name
    }

    pub fn surface_under_pointer(
        &mut self,
        pointer: &PointerHandle<Self>,
    ) -> Option<(WlSurface, Point<i32, Logical>)> {
        let pos = pointer.current_location();
        self.workspaces
            .current()
            .window_under(pos)
            .and_then(|(window, location)| {
                window
                    .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
                    .map(|(s, p)| (s, p + location))
            })
    }
}

pub struct ClientState;
impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}
