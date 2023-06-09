use std::{ffi::OsString, os::fd::AsRawFd, sync::Arc, time::Instant};

use smithay::{
    desktop::{Window, PopupManager, layer_map_for_output},
    input::{Seat, SeatState, keyboard::XkbConfig},
    reexports::{
        calloop::{generic::Generic, Interest, LoopSignal, Mode, PostAction, LoopHandle},
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            Display, DisplayHandle,
        },
    },
    utils::{Logical, Point},
    wayland::{
        compositor::CompositorState,
        data_device::DataDeviceState,
        output::OutputManagerState,
        shell::{xdg::{decoration::XdgDecorationState, XdgShellState}, wlr_layer::{WlrLayerShellState, Layer as WlrLayer}},
        shm::ShmState,
        socket::ListeningSocketSource, primary_selection::PrimarySelectionState,
    },
};
use tracing::warn;

use crate::{config::Config, utils::{workspaces::Workspaces, focus::FocusTarget}, ipc::{MagmaIpcManager, MagmaIpcHandler}, delegate_magma_ipc};

pub struct CalloopData<BackendData: Backend + 'static> {
    pub state: MagmaState<BackendData>,
    pub display: Display<MagmaState<BackendData>>,
}

pub trait Backend {
    fn seat_name(&self) -> String;
}

pub struct MagmaState<BackendData: Backend + 'static> {
    pub dh: DisplayHandle,
    pub backend_data: BackendData,
    pub loop_handle: LoopHandle<'static, CalloopData<BackendData>>,
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
    pub seat_state: SeatState<MagmaState<BackendData>>,
    pub data_device_state: DataDeviceState,
    pub primary_selection_state: PrimarySelectionState,
    pub popup_manager: PopupManager,
    pub layer_shell_state: WlrLayerShellState,
    pub seat: Seat<Self>,

    pub pointer_location: Point<f64, Logical>,

    pub ipc_manager: MagmaIpcManager,
}

impl<BackendData: Backend> MagmaState<BackendData> {
    pub fn new(
        mut loop_handle: LoopHandle<'static, CalloopData<BackendData>>,
        loop_signal: LoopSignal,
        display: &mut Display<MagmaState<BackendData>>,
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
        let primary_selection_state = PrimarySelectionState::new::<Self>(&dh);
        let layer_shell_state = WlrLayerShellState::new::<Self>(&dh);
        let seat_name = backend_data.seat_name();
        let mut seat = seat_state.new_wl_seat(&dh, seat_name.clone());
        let conf = config.xkb.clone();
        if let Err(err) = seat.add_keyboard((&conf).into(), 200, 25) {
            warn!(
                ?err,
                "Failed to load provided xkb config. Trying default...",
            );
            seat.add_keyboard(XkbConfig::default(), 200, 25)
                .expect("Failed to load xkb configuration files");
        }
        seat.add_pointer();

        let workspaces = Workspaces::new(config.workspaces);

        let socket_name = Self::init_wayland_listener(&mut loop_handle, display);

        let ipc_manager = MagmaIpcManager::new::<Self>(&dh);

        Self {
            loop_handle,
            dh,
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
            primary_selection_state,
            layer_shell_state,
            seat,
            pointer_location: Point::from((0.0, 0.0)),
            popup_manager: PopupManager::default(),
            ipc_manager,
        }
    }
    fn init_wayland_listener(
        handle: &mut LoopHandle<'static, CalloopData<BackendData>>,
        display: &mut Display<MagmaState<BackendData>>,
    ) -> OsString {
        // Creates a new listening socket, automatically choosing the next available `wayland` socket name.
        let listening_socket = ListeningSocketSource::new_auto().unwrap();

        // Get the name of the listening socket.
        // Clients will connect to this socket.
        let socket_name = listening_socket.socket_name().to_os_string();

        handle
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

delegate_magma_ipc!(@<BackendData: Backend + 'static> MagmaState<BackendData>);

impl<BackendData: Backend> MagmaIpcHandler for MagmaState<BackendData> {
    fn register_workspace(&mut self, workspace: crate::ipc::generated::workspaces::Workspaces) {
        self.ipc_manager.workspace_handles.push(workspace);
        self.ipc_manager.update_active_workspace(self.workspaces.current.into());
        self.ipc_manager.update_occupied_workspaces(&mut self.workspaces);
    }
}