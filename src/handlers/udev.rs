use std::{os::unix::prelude::FromRawFd, path::PathBuf};

use smithay::{
    backend::{
        allocator::{
            dmabuf::DmabufAllocator,
            gbm::{self, GbmAllocator, GbmBufferFlags},
        },
        drm::{self, DrmDeviceFd, DrmNode},
        egl::{EGLDevice, EGLDisplay},
        session::Session,
        udev::UdevEvent,
    },
    reexports::nix::fcntl::OFlag,
    utils::DeviceFd,
};

use crate::backends::udev::{Device, UdevData};

// Udev
impl UdevData {
    pub fn on_udev_event(&mut self, event: UdevEvent) {
        match event {
            UdevEvent::Added { device_id, path } => {
                if let Ok(node) = DrmNode::from_dev_id(device_id) {
                    self.on_device_added(node, path);
                }
            }
            UdevEvent::Changed { device_id } => {
                if let Ok(node) = DrmNode::from_dev_id(device_id) {
                    self.on_device_changed(node);
                }
            }
            UdevEvent::Removed { device_id } => {
                if let Ok(node) = DrmNode::from_dev_id(device_id) {
                    self.on_device_removed(node);
                }
            }
        }
    }

    fn on_device_added(&mut self, node: DrmNode, path: PathBuf) {
        let fd = self
            .session
            .open(
                &path,
                OFlag::O_RDWR | OFlag::O_CLOEXEC | OFlag::O_NOCTTY | OFlag::O_NONBLOCK,
            )
            .unwrap();

        let fd = DrmDeviceFd::new(unsafe { DeviceFd::from_raw_fd(fd) });

        let (drm, drm_notifier) = drm::DrmDevice::new(fd, false).unwrap();

        let gbm = gbm::GbmDevice::new(drm.device_fd().clone()).unwrap();
        let gbm_allocator = GbmAllocator::new(gbm.clone(), GbmBufferFlags::RENDERING);

        // Make sure display is dropped before we call add_node
        let render_node =
            match EGLDevice::device_for_display(&EGLDisplay::new(gbm.clone()).unwrap())
                .ok()
                .and_then(|x| x.try_get_render_node().ok().flatten())
            {
                Some(node) => node,
                None => node,
            };

        self.gpus
            .as_mut()
            .add_node(render_node, gbm.clone())
            .unwrap();

        self.handle
            .insert_source(drm_notifier, move |event, meta, calloopdata| {
                calloopdata
                    .state
                    .backend_data
                    .on_drm_event(node, event, meta)
            })
            .unwrap();

        self.devices.insert(
            node,
            Device {
                drm,
                gbm,
                gbm_allocator: DmabufAllocator(gbm_allocator),
                drm_scanner: Default::default(),
                surfaces: Default::default(),
                render_node,
            },
        );

        self.on_device_changed(node);
    }

    fn on_device_changed(&mut self, node: DrmNode) {
        if let Some(device) = self.devices.get_mut(&node) {
            for event in device.drm_scanner.scan_connectors(&device.drm) {
                self.on_connector_event(node, event);
            }
        }
    }

    fn on_device_removed(&mut self, node: DrmNode) {
        if let Some(device) = self.devices.get_mut(&node) {
            self.gpus.as_mut().remove_node(&device.render_node);
        }
    }
}
