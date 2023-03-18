use std::sync::Mutex;

use smithay::{
    delegate_xdg_shell,
    desktop::{Space, Window},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    wayland::{
        compositor::with_states,
        shell::xdg::{XdgShellHandler, XdgShellState, XdgToplevelSurfaceRoleAttributes},
    },
};

use crate::{state::HoloState, utils::tiling::bsp_layout};

impl XdgShellHandler for HoloState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: smithay::wayland::shell::xdg::ToplevelSurface) {
        let window = Window::new(surface);
        self.space.map_element(window, (0, 0), true);
    }

    fn new_popup(
        &mut self,
        surface: smithay::wayland::shell::xdg::PopupSurface,
        positioner: smithay::wayland::shell::xdg::PositionerState,
    ) {
        todo!()
    }

    fn grab(
        &mut self,
        surface: smithay::wayland::shell::xdg::PopupSurface,
        seat: smithay::reexports::wayland_server::protocol::wl_seat::WlSeat,
        serial: smithay::utils::Serial,
    ) {
        todo!()
    }
}

/// Should be called on `WlSurface::commit`
pub fn handle_commit(space: &mut Space<Window>, surface: &WlSurface) -> Option<()> {
    let window = space
        .elements()
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
        let layout = bsp_layout(space);
        let windows: Vec<_> = space.elements().cloned().collect();
        for (i, window) in windows.iter().enumerate() {
            space.map_element(window.clone(), layout[i].loc, false);
            let xdg_toplevel = window.toplevel();
            xdg_toplevel.with_pending_state(|state| {
                state.size = Some(layout[i].size);
            });
            xdg_toplevel.send_configure();
        }
    }

    Some(())
}

delegate_xdg_shell!(HoloState);
