use std::{ffi::OsString, os::fd::AsRawFd, sync::Arc, time::Instant};

use smithay::{
    desktop::{Window, PopupManager, layer_map_for_output},
    input::{Seat, SeatState},
    reexports::{
        calloop::{generic::Generic, EventLoop, Interest, LoopSignal, Mode, PostAction},
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            Display,
        },
    },
    utils::{Logical, Point},
    wayland::{
        compositor::CompositorState,
        data_device::DataDeviceState,
        output::OutputManagerState,
        shell::{xdg::{decoration::XdgDecorationState, XdgShellState}, wlr_layer::{WlrLayerShellState, Layer as WlrLayer}},
        shm::ShmState,
        socket::ListeningSocketSource,
    },
};

use crate::{config::Config, utils::{workspaces::Workspaces, focus::FocusTarget}};

pub struct CalloopData<BackendData: Backend + 'static> {
    pub state: HoloState<BackendData>,
    pub display: Display<HoloState<BackendData>>,
}

pub trait Backend {
    fn seat_name(&self) -> String;
}

pub struct HoloState<BackendData: Backend + 'static> {
    pub backend_data: BackendData,
    pub config: Config,
    pub start_time: Instant,
    pub socket_name: OsString,
    pub seat_name: String,
    pub loop_signal: LoopSignal,
    pub workspaces: Workspaces,

    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub xdg_decoration_state: XdgDecorationState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<HoloState<BackendData>>,
    pub data_device_state: DataDeviceState,
    pub popup_manager: PopupManager,
    pub layer_shell_state: WlrLayerShellState,
    pub seat: Seat<Self>,

    pub pointer_location: Point<f64, Logical>,
}

impl<BackendData: Backend> HoloState<BackendData> {
    pub fn new(
        event_loop: &mut EventLoop<CalloopData<BackendData>>,
        display: &mut Display<HoloState<BackendData>>,
        backend_data: BackendData,
    ) -> Self {
        let start_time = Instant::now();

        let dh = display.handle();

        let config = Config::load();

        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let xdg_decoration_state = XdgDecorationState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let layer_shell_state = WlrLayerShellState::new::<Self>(&dh);
        let seat_name = backend_data.seat_name();
        let mut seat = seat_state.new_wl_seat(&dh, seat_name.clone());
        seat.add_keyboard(Default::default(), 600, 25).unwrap();
        seat.add_pointer();

        let workspaces = Workspaces::new(config.workspaces);

        let socket_name = Self::init_wayland_listener(event_loop, display);

        let loop_signal = event_loop.get_signal();

        Self {
            backend_data,
            config,
            start_time,
            seat_name,
            socket_name,
            workspaces,
            compositor_state,
            xdg_shell_state,
            xdg_decoration_state,
            loop_signal,
            shm_state,
            output_manager_state,
            seat_state,
            data_device_state,
            layer_shell_state,
            seat,
            pointer_location: Point::from((0.0, 0.0)),
            popup_manager: PopupManager::default(),
        }
    }
    fn init_wayland_listener(
        event_loop: &mut EventLoop<CalloopData<BackendData>>,
        display: &mut Display<HoloState<BackendData>>,
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

    pub fn window_under(&mut self) -> Option<(Window, Point<i32, Logical>)> {
        let pos = self.pointer_location;
        self.workspaces
            .current()
            .window_under(pos)
            .map(|(w, p)| (w.clone(), p))
    }
    pub fn surface_under(&self) -> Option<(FocusTarget, Point<i32, Logical>)> {
        let pos = self.pointer_location;
        let output = self.workspaces.current().outputs().find(|o| {
            let geometry = self.workspaces.current().output_geometry(o).unwrap();
            geometry.contains(pos.to_i32_round())
        })?;
        let output_geo = self.workspaces.current().output_geometry(output).unwrap();
        let layers = layer_map_for_output(output);

        let mut under = None;
        if let Some(layer) = layers
            .layer_under(WlrLayer::Overlay, pos)
            .or_else(|| layers.layer_under(WlrLayer::Top, pos))
        {
            let layer_loc = layers.layer_geometry(layer).unwrap().loc;
            under = Some((layer.clone().into(), output_geo.loc + layer_loc))
        } else if let Some((window, location)) = self.workspaces.current().window_under(pos) {
            under = Some((window.clone().into(), location));
        } else if let Some(layer) = layers
            .layer_under(WlrLayer::Bottom, pos)
            .or_else(|| layers.layer_under(WlrLayer::Background, pos))
        {
            let layer_loc = layers.layer_geometry(layer).unwrap().loc;
            under = Some((layer.clone().into(), output_geo.loc + layer_loc));
        };
        under
    }
}

pub struct ClientState;
impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}
