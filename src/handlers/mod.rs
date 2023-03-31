pub mod compositor;
pub mod input;
pub mod xdg_shell;

//
// Wl Seat
//

use smithay::desktop::Window;
use smithay::input::{SeatHandler, SeatState};

use smithay::wayland::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, ServerDndGrabHandler,
};
use smithay::{delegate_data_device, delegate_output, delegate_seat};

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
