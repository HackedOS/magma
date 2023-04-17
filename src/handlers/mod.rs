pub mod compositor;
pub mod input;
pub mod xdg_shell;

//
// Wl Seat
//

use smithay::desktop::{layer_map_for_output, LayerSurface};
use smithay::input::{SeatHandler, SeatState};

use smithay::output::Output;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::wayland::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, ServerDndGrabHandler,
};
use smithay::wayland::shell::wlr_layer::{WlrLayerShellHandler, WlrLayerShellState, LayerSurface as WlrLayerSurface, Layer};
use smithay::{delegate_data_device, delegate_output, delegate_seat, delegate_layer_shell};

use crate::state::{Backend, HoloState};
use crate::utils::focus::FocusTarget;

impl<BackendData: Backend> SeatHandler for HoloState<BackendData> {
    type KeyboardFocus = FocusTarget;
    type PointerFocus = FocusTarget;

    fn seat_state(&mut self) -> &mut SeatState<HoloState<BackendData>> {
        &mut self.seat_state
    }

    fn cursor_image(
        &mut self,
        _seat: &smithay::input::Seat<Self>,
        _image: smithay::input::pointer::CursorImageStatus,
    ) {
    }
    fn focus_changed(&mut self, _seat: &smithay::input::Seat<Self>, _focused: Option<&FocusTarget>) {}
}

delegate_seat!(@<BackendData: Backend + 'static> HoloState<BackendData>);

//
// Wl Data Device
//

impl<BackendData: Backend> DataDeviceHandler for HoloState<BackendData> {
    fn data_device_state(&self) -> &smithay::wayland::data_device::DataDeviceState {
        &self.data_device_state
    }
}

impl<BackendData: Backend> ClientDndGrabHandler for HoloState<BackendData> {}
impl<BackendData: Backend> ServerDndGrabHandler for HoloState<BackendData> {}

delegate_data_device!(@<BackendData: Backend + 'static> HoloState<BackendData>);

//
// Wl Output & Xdg Output
//

delegate_output!(@<BackendData: Backend + 'static> HoloState<BackendData>);

impl<BackendData: Backend> WlrLayerShellHandler for HoloState<BackendData>{
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: WlrLayerSurface,
        output: Option<WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        let output = output.as_ref().and_then(Output::from_resource).unwrap_or_else(|| {
            self.workspaces.current().outputs().next().unwrap().clone()
        });
        let mut map = layer_map_for_output(&output);
        map.map_layer(&LayerSurface::new(surface, namespace)).unwrap();
    }

    fn layer_destroyed(&mut self, surface: WlrLayerSurface) {
        if let Some((mut map, layer)) = self.workspaces.outputs().find_map(|o| {
            let map = layer_map_for_output(o);
            let layer = map
                .layers()
                .find(|&layer| layer.layer_surface() == &surface)
                .cloned();
            layer.map(|layer| (map, layer))
        }) {
            map.unmap_layer(&layer);
        }
    }
}

delegate_layer_shell!(@<BackendData: Backend + 'static> HoloState<BackendData>);