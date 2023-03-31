use std::sync::Mutex;

use smithay::{
    delegate_xdg_decoration, delegate_xdg_shell,
    desktop::Window,
    reexports::{
        wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode,
        wayland_server::protocol::{wl_seat::WlSeat, wl_surface::WlSurface},
    },
    utils::Serial,
    wayland::{
        compositor::with_states,
        shell::xdg::{
            decoration::XdgDecorationHandler, PopupSurface, PositionerState, ToplevelSurface,
            XdgShellHandler, XdgShellState, XdgToplevelSurfaceRoleAttributes,
        },
    },
};

use crate::{
    state::{Backend, HoloState},
    utils::{
        tiling::{bsp_layout, WindowLayoutEvent},
        workspaces::Workspaces,
    },
};

impl<BackendData: Backend> XdgShellHandler for HoloState<BackendData> {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new(surface);
        bsp_layout(
            self.workspaces.current_mut(),
            window,
            WindowLayoutEvent::Added,
            self.config.gaps,
        );
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
    }
    fn new_popup(&mut self, _surface: PopupSurface, _positioner: PositionerState) {
        todo!()
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: WlSeat, _serial: Serial) {
        todo!()
    }
}

delegate_xdg_shell!(@<BackendData: Backend + 'static> HoloState<BackendData>);

/// Should be called on `WlSurface::commit`
pub fn handle_commit(workspaces: &Workspaces, surface: &WlSurface) -> Option<()> {
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

    Some(())
}

// Disable decorations
impl<BackendData: Backend> XdgDecorationHandler for HoloState<BackendData> {
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

delegate_xdg_decoration!(@<BackendData: Backend + 'static> HoloState<BackendData>);
