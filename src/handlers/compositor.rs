use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    delegate_compositor, delegate_shm,
    reexports::wayland_server::protocol::{wl_buffer::WlBuffer, wl_surface::WlSurface},
    wayland::{
        buffer::BufferHandler,
        compositor::{get_parent, is_sync_subsurface, CompositorHandler, CompositorState},
        shm::ShmHandler,
    },
};

use crate::state::{Backend, HoloState};

use super::xdg_shell;

impl<BackendData: Backend> CompositorHandler for HoloState<BackendData> {
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
                .workspaces
                .all_windows()
                .find(|w| w.toplevel().wl_surface() == &root)
            {
                window.on_commit();
            }
        };

        //Shell Commits here
        xdg_shell::handle_commit(&mut self.workspaces, surface);
    }
}

impl<BackendData: Backend> BufferHandler for HoloState<BackendData> {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}

impl<BackendData: Backend> ShmHandler for HoloState<BackendData> {
    fn shm_state(&self) -> &smithay::wayland::shm::ShmState {
        &self.shm_state
    }
}

delegate_compositor!(@<BackendData: Backend + 'static> HoloState<BackendData>);
delegate_shm!(@<BackendData: Backend + 'static> HoloState<BackendData>);
