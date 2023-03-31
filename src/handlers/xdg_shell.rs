use std::{cell::RefCell, rc::Rc, sync::Mutex};

use smithay::{
    delegate_xdg_decoration, delegate_xdg_shell,
    desktop::{Space, Window},
    reexports::{
        wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode,
        wayland_server::protocol::{wl_seat::WlSeat, wl_surface::WlSurface},
    },
    utils::{Rectangle, Serial},
    wayland::{
        compositor::with_states,
        shell::xdg::{
            decoration::XdgDecorationHandler, PopupSurface, PositionerState, ToplevelSurface,
            XdgShellHandler, XdgShellState, XdgToplevelSurfaceData,
            XdgToplevelSurfaceRoleAttributes,
        },
    },
};

use crate::{
    state::HoloState,
    utils::workspaces::{HoloWindow, Workspace},
};

impl XdgShellHandler for HoloState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new(surface);
        let size = self
            .workspace
            .outputs()
            .next()
            .unwrap()
            .current_mode()
            .unwrap()
            .size
            .to_logical(1);
        self.workspace
            .add_window(Rc::from(RefCell::from(HoloWindow {
                window,
                rec: Rectangle {
                    loc: (0, 0).into(),
                    size,
                },
            })));
    }

    fn new_popup(&mut self, surface: PopupSurface, positioner: PositionerState) {
        todo!()
    }

    fn grab(&mut self, surface: PopupSurface, seat: WlSeat, serial: Serial) {
        todo!()
    }
}

delegate_xdg_shell!(HoloState);

/// Should be called on `WlSurface::commit`
pub fn handle_commit(workspace: &Workspace, surface: &WlSurface) -> Option<()> {
    if let Some(window) = workspace
        .windows()
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
impl XdgDecorationHandler for HoloState {
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

delegate_xdg_decoration!(HoloState);
