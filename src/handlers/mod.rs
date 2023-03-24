use smithay::{
    delegate_data_device, delegate_output, delegate_seat,
    input::SeatHandler,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    wayland::data_device::{
        ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
    },
};

use crate::state::{Backend, HoloState};

mod compositor;
mod drm;
mod input;
mod udev;
mod xdg_shell;

impl<BackendData: Backend> SeatHandler for HoloState<BackendData> {
    type KeyboardFocus = WlSurface;

    type PointerFocus = WlSurface;

    fn seat_state(&mut self) -> &mut smithay::input::SeatState<Self> {
        &mut self.seat_state
    }
}

delegate_seat!(@<BackendData: Backend + 'static> HoloState<BackendData>);

impl<BackendData: Backend> DataDeviceHandler for HoloState<BackendData> {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}
impl<BackendData: Backend> ClientDndGrabHandler for HoloState<BackendData> {}

impl<BackendData: Backend> ServerDndGrabHandler for HoloState<BackendData> {}

delegate_data_device!(@<BackendData: Backend + 'static> HoloState<BackendData>);

delegate_output!(@<BackendData: Backend + 'static> HoloState<BackendData>);
