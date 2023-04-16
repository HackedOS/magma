pub mod compositor;
pub mod input;
pub mod xdg_shell;

//
// Wl Seat
//

use smithay::desktop::{Window, layer_map_for_output, LayerSurface};
use smithay::input::{SeatHandler, SeatState};

use smithay::output::Output;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::wayland::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, ServerDndGrabHandler,
};
use smithay::wayland::shell::wlr_layer::{WlrLayerShellHandler, WlrLayerShellState, LayerSurface as WlrLayerSurface, Layer};
use smithay::{delegate_data_device, delegate_output, delegate_seat, delegate_layer_shell};

use crate::state::{Backend, HoloState};

impl<BackendData: Backend> SeatHandler for HoloState<BackendData> {
    type KeyboardFocus = Window;
    type PointerFocus = Window;

    fn seat_state(&mut self) -> &mut SeatState<HoloState<BackendData>> {
        &mut self.seat_state
    }

    fn cursor_image(
        &mut self,
        _seat: &smithay::input::Seat<Self>,
        _image: smithay::input::pointer::CursorImageStatus,
    ) {
    }
    fn focus_changed(&mut self, _seat: &smithay::input::Seat<Self>, _focused: Option<&Window>) {}
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
            self.workspaces.current_mut().outputs().next().unwrap().clone()
        });
        let mut map = layer_map_for_output(&output);
        map.map_layer(&LayerSurface::new(surface, namespace)).unwrap();
    }
}

delegate_layer_shell!(@<BackendData: Backend + 'static> HoloState<BackendData>);