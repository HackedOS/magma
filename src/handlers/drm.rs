use smithay_drm_extras::drm_scanner::{self, DrmScanEvent};

use smithay::backend::drm::{self, DrmNode};

use crate::{backends::udev::UdevData, surface::Surface};

// Drm
impl UdevData {
    pub fn on_drm_event(
        &mut self,
        node: DrmNode,
        event: drm::DrmEvent,
        _meta: &mut Option<drm::DrmEventMetadata>,
    ) {
        match event {
            drm::DrmEvent::VBlank(crtc) => {
                if let Some(device) = self.devices.get_mut(&node) {
                    if let Some(surface) = device.surfaces.get_mut(&crtc) {
                        let mut renderer = if self.primary_gpu == device.render_node {
                            self.gpus.single_renderer(&device.render_node).unwrap()
                        } else {
                            self.gpus
                                .renderer(
                                    &self.primary_gpu,
                                    &device.render_node,
                                    &mut device.gbm_allocator,
                                    surface.gbm_surface.format(),
                                )
                                .unwrap()
                        };

                        surface.gbm_surface.frame_submitted().unwrap();
                        surface.next_buffer(&mut renderer);
                    }
                }
            }
            drm::DrmEvent::Error(_) => {}
        }
    }

    pub fn on_connector_event(&mut self, node: DrmNode, event: drm_scanner::DrmScanEvent) {
        let device = if let Some(device) = self.devices.get_mut(&node) {
            device
        } else {
            return;
        };

        match event {
            DrmScanEvent::Connected {
                connector,
                crtc: Some(crtc),
            } => {
                let mut renderer = self.gpus.single_renderer(&device.render_node).unwrap();

                let mut surface = Surface::new(
                    crtc,
                    &connector,
                    renderer
                        .as_mut()
                        .egl_context()
                        .dmabuf_render_formats()
                        .clone(),
                    &device.drm,
                    device.gbm.clone(),
                );

                surface.next_buffer(renderer.as_mut());

                device.surfaces.insert(crtc, surface);
            }
            DrmScanEvent::Disconnected {
                crtc: Some(crtc), ..
            } => {
                device.surfaces.remove(&crtc);
            }
            _ => {}
        }
    }
}
