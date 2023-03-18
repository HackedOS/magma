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
    utils::{tiling::bsp_layout, workspace::Workspace},
};

impl XdgShellHandler for HoloState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: smithay::wayland::shell::xdg::ToplevelSurface) {
        let window = Window::new(surface);
        self.workspace.add_window(window, (0, 0));
        let layout = bsp_layout(&self.workspace);
        let windows: Vec<_> = self.workspace.windows().cloned().collect();
        for (i, window) in windows.iter().enumerate() {
            self.workspace.add_window(window.clone(), layout[i].loc);
            let xdg_toplevel = window.toplevel();
            xdg_toplevel.with_pending_state(|state| {
                state.size = Some(layout[i].size);
            });
            xdg_toplevel.send_configure();
        }
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        let window = self
            .workspace
            .windows()
            .find(|w| w.toplevel() == &surface)
            .cloned()
            .unwrap();
        self.workspace.remove_window(&window);
        let layout = bsp_layout(&self.workspace);
        let windows: Vec<_> = self.workspace.windows().cloned().collect();
        for (i, window) in windows.iter().enumerate() {
            self.workspace.add_window(window.clone(), layout[i].loc);
            let xdg_toplevel = window.toplevel();
            xdg_toplevel.with_pending_state(|state| {
                state.size = Some(layout[i].size);
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
pub fn handle_commit(workspace: &mut Workspace, surface: &WlSurface) -> Option<()> {
    let window = workspace
        .windows()
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
