use std::sync::Mutex;

use smithay::{
    delegate_xdg_decoration, delegate_xdg_shell,
    desktop::Window,
    reexports::{
        wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode,
        wayland_server::protocol::wl_surface::WlSurface,
    },
    wayland::{
        compositor::with_states,
        shell::xdg::{
            decoration::XdgDecorationHandler, ToplevelSurface, XdgShellHandler, XdgShellState,
            XdgToplevelSurfaceRoleAttributes,
        },
    },
};

use crate::{
    state::HoloState,
    utils::{
        tiling::{bsp_layout, WindowLayoutEvent},
        workspace::{self, Workspaces},
    },
};

impl XdgShellHandler for HoloState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: smithay::wayland::shell::xdg::ToplevelSurface) {
        let window = Window::new(surface);
        bsp_layout(
            &mut self.workspaces.current(),
            WindowLayoutEvent::Added,
            window.clone(),
        );

        let workspace = self.workspaces.current();
        let windows: Vec<_> = workspace.windows().cloned().collect();
        for window in windows.iter() {
            let geometry = workspace.geometry(&window);
            let xdg_toplevel = window.toplevel();
            xdg_toplevel.with_pending_state(|state| {
                state.size = Some(geometry.size);
            });
            xdg_toplevel.send_configure();
        }
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        let window = self
            .workspaces
            .all_windows()
            .find(|w| w.toplevel() == &surface)
            .cloned()
            .unwrap();

        let workspace = self.workspaces.workspace_from_window(&window).unwrap();
        bsp_layout(workspace, WindowLayoutEvent::Removed, window.clone());
        let windows: Vec<_> = workspace.windows().cloned().collect();
        for window in windows.iter() {
            let geometry = workspace.geometry(&window);
            let xdg_toplevel = window.toplevel();
            xdg_toplevel.with_pending_state(|state| {
                state.size = Some(geometry.size);
            });
            xdg_toplevel.send_configure();
        }
    }

    fn new_popup(
        &mut self,
        _surface: smithay::wayland::shell::xdg::PopupSurface,
        _positioner: smithay::wayland::shell::xdg::PositionerState,
    ) {
        todo!()
    }

    fn grab(
        &mut self,
        _surface: smithay::wayland::shell::xdg::PopupSurface,
        _seat: smithay::reexports::wayland_server::protocol::wl_seat::WlSeat,
        _serial: smithay::utils::Serial,
    ) {
        todo!()
    }
}

/// Should be called on `WlSurface::commit`
pub fn handle_commit(workspaces: &mut Workspaces, surface: &WlSurface) -> Option<()> {
    let window = workspaces
        .all_windows()
        .find(|w| w.toplevel().wl_surface() == surface)
        .cloned()?;

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

    Some(())
}

delegate_xdg_shell!(HoloState);

// Disable decorations
impl XdgDecorationHandler for HoloState {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            // Advertise server side decoration
            state.decoration_mode = Some(Mode::ServerSide);
        });
        toplevel.send_configure();
    }

    fn request_mode(&mut self, _toplevel: ToplevelSurface, _mode: Mode) {}

    fn unset_mode(&mut self, _toplevel: ToplevelSurface) {}
}

delegate_xdg_decoration!(HoloState);
