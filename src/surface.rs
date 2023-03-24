use std::collections::HashSet;

use smithay_drm_extras::edid::EdidInfo;

use smithay::{
    backend::{
        allocator::Format,
        allocator::{
            dmabuf::Dmabuf,
            gbm::{self, GbmAllocator, GbmBufferFlags},
        },
        drm::{self, DrmDeviceFd, GbmBufferedSurface},
        renderer::{
            damage::DamageTrackedRenderer, element::memory::MemoryRenderBufferRenderElement, Bind,
            ImportMem, Renderer,
        },
    },
    output::{Mode as WlMode, Output, PhysicalProperties},
    reexports::drm::control::{connector, crtc, ModeTypeFlags},
    utils::Transform,
};

pub struct Surface {
    pub gbm_surface: GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, ()>,
    pub output: Output,
    pub damage_tracked_renderer: DamageTrackedRenderer,
}

impl Surface {
    pub fn new(
        crtc: crtc::Handle,
        connector: &connector::Info,
        formats: HashSet<Format>,
        drm: &drm::DrmDevice,
        gbm: gbm::GbmDevice<DrmDeviceFd>,
    ) -> Self {
        let mode_id = connector
            .modes()
            .iter()
            .position(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
            .unwrap_or(0);

        let drm_mode = connector.modes()[mode_id];

        let drm_surface = drm
            .create_surface(crtc, drm_mode, &[connector.handle()])
            .unwrap();

        let gbm_surface = GbmBufferedSurface::new(
            drm_surface,
            GbmAllocator::new(gbm, GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT),
            formats,
        )
        .unwrap();

        let name = smithay_drm_extras::format_connector_name(connector);

        let (make, model) = EdidInfo::for_connector(drm, connector.handle())
            .map(|info| (info.manufacturer, info.model))
            .unwrap_or_else(|| ("Unknown".into(), "Unknown".into()));

        let (w, h) = connector.size().unwrap_or((0, 0));
        let output = Output::new(
            name,
            PhysicalProperties {
                size: (w as i32, h as i32).into(),
                subpixel: smithay::output::Subpixel::Unknown,
                make,
                model,
            },
        );

        let output_mode = WlMode::from(drm_mode);
        output.set_preferred(output_mode);
        output.change_current_state(
            Some(output_mode),
            Some(Transform::Normal),
            Some(smithay::output::Scale::Integer(1)),
            None,
        );

        let damage_tracked_renderer = DamageTrackedRenderer::from_output(&output);

        Self {
            gbm_surface,
            output,
            damage_tracked_renderer,
        }
    }

    pub fn next_buffer<R>(&mut self, renderer: &mut R)
    where
        R: Renderer + ImportMem + Bind<Dmabuf>,
        R::TextureId: 'static,
    {
        let (dmabuf, age) = self.gbm_surface.next_buffer().unwrap();
        renderer.bind(dmabuf).unwrap();

        self.damage_tracked_renderer
            .render_output::<MemoryRenderBufferRenderElement<R>, _>(
                renderer,
                age as usize,
                &[],
                [1.0, 0.0, 0.0, 1.0],
            )
            .unwrap();

        self.gbm_surface.queue_buffer(None, ()).unwrap();
    }
}
