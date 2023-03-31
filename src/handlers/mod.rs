pub mod compositor;
pub mod xdg_shell;

//
// Wl Seat
//

use smithay::input::{SeatHandler, SeatState};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, ServerDndGrabHandler,
};
use smithay::{delegate_data_device, delegate_output, delegate_seat};

use crate::state::HoloState;

impl SeatHandler for HoloState {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<HoloState> {
        &mut self.seat_state
    }

    fn cursor_image(
        &mut self,
        _seat: &smithay::input::Seat<Self>,
        _image: smithay::input::pointer::CursorImageStatus,
    ) {
    }
    fn focus_changed(&mut self, _seat: &smithay::input::Seat<Self>, _focused: Option<&WlSurface>) {}
}

delegate_seat!(HoloState);

//
// Wl Data Device
//

impl DataDeviceHandler for HoloState {
    fn data_device_state(&self) -> &smithay::wayland::data_device::DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for HoloState {}
impl ServerDndGrabHandler for HoloState {}

delegate_data_device!(HoloState);

//
// Wl Output & Xdg Output
//

delegate_output!(HoloState);
