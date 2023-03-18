use smithay::{
    delegate_data_device, delegate_output, delegate_seat,
    input::SeatHandler,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    wayland::data_device::{
        ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
    },
};

use crate::state::HoloState;

mod compositor;
mod xdg_shell;

impl SeatHandler for HoloState {
    type KeyboardFocus = WlSurface;

    type PointerFocus = WlSurface;

    fn seat_state(&mut self) -> &mut smithay::input::SeatState<Self> {
        &mut self.seat_state
    }
}

delegate_seat!(HoloState);

impl DataDeviceHandler for HoloState {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}
impl ClientDndGrabHandler for HoloState {}

impl ServerDndGrabHandler for HoloState {}

delegate_data_device!(HoloState);

delegate_output!(HoloState);
