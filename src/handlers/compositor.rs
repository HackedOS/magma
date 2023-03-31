use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    delegate_compositor, delegate_shm,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    wayland::{
        buffer::BufferHandler,
        compositor::{get_parent, is_sync_subsurface, CompositorHandler, CompositorState},
        shm::{ShmHandler, ShmState},
    },
};

use crate::state::HoloState;

use super::xdg_shell;

impl CompositorHandler for HoloState {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler(surface);
        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            if let Some(window) = self
                .workspace
                .windows()
                .find(|w| w.toplevel().wl_surface() == &root)
            {
                window.on_commit();
            }
        };

        xdg_shell::handle_commit(&self.workspace, surface);
    }
}

delegate_compositor!(HoloState);

impl BufferHandler for HoloState {
    fn buffer_destroyed(
        &mut self,
        _buffer: &smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer,
    ) {
    }
}

impl ShmHandler for HoloState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

delegate_shm!(HoloState);
