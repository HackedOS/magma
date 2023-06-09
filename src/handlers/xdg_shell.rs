use std::sync::Mutex;

use smithay::{
    delegate_xdg_decoration, delegate_xdg_shell,
    desktop::{Window, PopupKind, PopupManager, layer_map_for_output, WindowSurfaceType},
    reexports::{
        wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode,
        wayland_server::protocol::{wl_seat::WlSeat, wl_surface::WlSurface},
    },
    utils::Serial,
    wayland::{
        compositor::with_states,
        shell::{xdg::{
            decoration::XdgDecorationHandler, PopupSurface, PositionerState, ToplevelSurface,
            XdgShellHandler, XdgShellState, XdgToplevelSurfaceRoleAttributes, XdgPopupSurfaceData,
        }, wlr_layer::LayerSurfaceData},
    },
};
use tracing::warn;

use crate::{
    state::{Backend, MagmaState},
    utils::{
        tiling::{bsp_layout, WindowLayoutEvent},
        workspaces::Workspaces, focus::FocusTarget,
    },
};

impl<BackendData: Backend> XdgShellHandler for MagmaState<BackendData> {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new(surface);
        bsp_layout(
            self.workspaces.current_mut(),
            window.clone(),
            WindowLayoutEvent::Added,
            self.config.gaps,
        );
        self.set_input_focus(FocusTarget::Window(window));
        self.ipc_manager.update_occupied_workspaces(&mut self.workspaces);
    }
    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        let window = self
            .workspaces
            .all_windows()
            .find(|w| w.toplevel() == &surface)
            .unwrap()
            .clone();

        let workspace = self.workspaces.workspace_from_window(&window).unwrap();
        bsp_layout(
            workspace,
            window,
            WindowLayoutEvent::Removed,
            self.config.gaps,
        );

        self.set_input_focus_auto();
        self.ipc_manager.update_occupied_workspaces(&mut self.workspaces);
    }
    fn new_popup(&mut self, surface: PopupSurface, positioner: PositionerState) {
        surface.with_pending_state(|state| {

            state.geometry = positioner.get_geometry();
        });
        if let Err(err) = self.popup_manager.track_popup(PopupKind::from(surface)) {
            warn!("Failed to track popup: {}", err);
        }
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: WlSeat, _serial: Serial) {
        // TODO
    }
}

delegate_xdg_shell!(@<BackendData: Backend + 'static> MagmaState<BackendData>);

/// Should be called on `WlSurface::commit`
pub fn handle_commit(workspaces: &Workspaces, surface: &WlSurface, popup_manager: &PopupManager) -> Option<()> {
    if let Some(window) = workspaces
        .all_windows()
        .find(|w| w.toplevel().wl_surface() == surface)
    {
        let initial_configure_sent = with_states(surface, |states| {
            states
                .data_map
                .get::<Mutex<XdgToplevelSurfaceRoleAttributes>>()
                .unwrap()
                .lock()
                .unwrap()
                .initial_configure_sent
        });
        if !initial_configure_sent {
            window.toplevel().send_configure();
        }
    }

    if let Some(popup) = popup_manager.find_popup(surface) {
        let PopupKind::Xdg(ref popup) = popup;
        let initial_configure_sent = with_states(surface, |states| {
            states
                .data_map
                .get::<XdgPopupSurfaceData>()
                .unwrap()
                .lock()
                .unwrap()
                .initial_configure_sent
        });
        if !initial_configure_sent {
            // NOTE: This should never fail as the initial configure is always
            // allowed.
            popup.send_configure().expect("initial configure failed");
        }
    };

    if let Some(output) = workspaces.current().outputs().find(|o| {
        let map = layer_map_for_output(o);
        map.layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
            .is_some()
    }) {
        let initial_configure_sent = with_states(surface, |states| {
            states
                .data_map
                .get::<LayerSurfaceData>()
                .unwrap()
                .lock()
                .unwrap()
                .initial_configure_sent
        });
        let mut map = layer_map_for_output(output);

        // arrange the layers before sending the initial configure
        // to respect any size the client may have sent
        map.arrange();
        // send the initial configure if relevant
        if !initial_configure_sent {
            let layer = map
                .layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
                .unwrap();

            layer.layer_surface().send_configure();
        }
    };

    Some(())
}

// Disable decorations
impl<BackendData: Backend> XdgDecorationHandler for MagmaState<BackendData> {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            // Advertise server side decoration
            state.decoration_mode = Some(Mode::ServerSide);
        });
        toplevel.send_configure();
    }

    fn request_mode(
        &mut self,
        _toplevel: ToplevelSurface,
        _mode: smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode,
    ) {
    }

    fn unset_mode(&mut self, _toplevel: ToplevelSurface) {}
}

delegate_xdg_decoration!(@<BackendData: Backend + 'static> MagmaState<BackendData>);
