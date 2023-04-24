use smithay::reexports::wayland_server::Dispatch;

use super::{generated::workspaces::Workspaces, MagmaIpcManager, MagmaIpcHandler};

impl<D> Dispatch<Workspaces, (), D> for MagmaIpcManager
where
    D: Dispatch<Workspaces, ()>,
    D: MagmaIpcHandler,
    D: 'static, {
    fn request(
        _state: &mut D,
        _client: &smithay::reexports::wayland_server::Client,
        _resource: &Workspaces,
        _request: <Workspaces as smithay::reexports::wayland_server::Resource>::Request,
        _data: &(),
        _dhandle: &smithay::reexports::wayland_server::DisplayHandle,
        _data_init: &mut smithay::reexports::wayland_server::DataInit<'_, D>,
    ) {
        
    }
}
