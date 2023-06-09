pub mod compositor;
pub mod input;
pub mod xdg_shell;

//
// Wl Seat
//

use smithay::desktop::{layer_map_for_output, LayerSurface};
use smithay::input::{SeatHandler, SeatState};

use smithay::output::Output;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::wayland::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, ServerDndGrabHandler, set_data_device_focus,
};
use smithay::wayland::primary_selection::{PrimarySelectionHandler, set_primary_focus};
use smithay::wayland::seat::WaylandFocus;
use smithay::wayland::shell::wlr_layer::{WlrLayerShellHandler, WlrLayerShellState, LayerSurface as WlrLayerSurface, Layer};
use smithay::{delegate_data_device, delegate_output, delegate_seat, delegate_layer_shell, delegate_primary_selection};

use crate::state::{Backend, MagmaState};
use crate::utils::focus::FocusTarget;

impl<BackendData: Backend> SeatHandler for MagmaState<BackendData> {
    type KeyboardFocus = FocusTarget;
    type PointerFocus = FocusTarget;

    fn seat_state(&mut self) -> &mut SeatState<MagmaState<BackendData>> {
        &mut self.seat_state
    }

    fn cursor_image(
        &mut self,
        _seat: &smithay::input::Seat<Self>,
        _image: smithay::input::pointer::CursorImageStatus,
    ) {
    }
    fn focus_changed(&mut self, seat: &smithay::input::Seat<Self>, focused: Option<&FocusTarget>) {
        let dh = &self.dh;

        let focus = focused
            .and_then(WaylandFocus::wl_surface)
            .and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, focus.clone());
        set_primary_focus(dh, seat, focus);

        if let Some(focus_target) = focused {
            match focus_target {
                FocusTarget::Window(w) => {
                    for window in self.workspaces.all_windows(){
                        if window.eq(w){
                            window.set_activated(true);
                        }else{
                            window.set_activated(false);
                        }
                        window.toplevel().send_configure();
                    }
                },
                FocusTarget::LayerSurface(_) => {
                    for window in self.workspaces.all_windows() {
                    window.set_activated(false);
                    window.toplevel().send_configure();
                    }
                },
                FocusTarget::Popup(_) => {},
            };
        }
    }
}

delegate_seat!(@<BackendData: Backend + 'static> MagmaState<BackendData>);

//
// Wl Data Device
//

impl<BackendData: Backend> DataDeviceHandler for MagmaState<BackendData> {
    fn data_device_state(&self) -> &smithay::wayland::data_device::DataDeviceState {
        &self.data_device_state
    }
}

impl<BackendData: Backend> ClientDndGrabHandler for MagmaState<BackendData> {}
impl<BackendData: Backend> ServerDndGrabHandler for MagmaState<BackendData> {}

delegate_data_device!(@<BackendData: Backend + 'static> MagmaState<BackendData>);

impl<BackendData:Backend> PrimarySelectionHandler for MagmaState<BackendData> {
    fn primary_selection_state(&self) -> &smithay::wayland::primary_selection::PrimarySelectionState {
        &self.primary_selection_state
    }
}

delegate_primary_selection!(@<BackendData: Backend + 'static> MagmaState<BackendData>);

//
// Wl Output & Xdg Output
//

delegate_output!(@<BackendData: Backend + 'static> MagmaState<BackendData>);

impl<BackendData: Backend> WlrLayerShellHandler for MagmaState<BackendData>{
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
        let layer_surface = LayerSurface::new(surface, namespace);
        map.map_layer(&layer_surface).unwrap();
        self.set_input_focus(FocusTarget::LayerSurface(layer_surface))
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
        self.set_input_focus_auto()
    }
}

delegate_layer_shell!(@<BackendData: Backend + 'static> MagmaState<BackendData>);