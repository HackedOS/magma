use std::{
    cell::{Ref, RefCell},
    rc::Rc,
};

use smithay::{
    backend::renderer::{
        element::{surface::WaylandSurfaceRenderElement, AsRenderElements},
        ImportAll, Renderer, Texture,
    },
    desktop::{space::SpaceElement, Window},
    output::Output,
    utils::{Logical, Point, Rectangle, Scale, Transform},
};

#[derive(Debug, PartialEq, Clone)]
pub struct HoloWindow {
    pub window: Window,
    pub rec: Rectangle<i32, Logical>,
}
impl HoloWindow {
    fn bbox(&self) -> Rectangle<i32, Logical> {
        let mut bbox = self.window.bbox();
        bbox.loc += self.rec.loc - self.window.geometry().loc;
        bbox
    }

    fn render_location(&self) -> Point<i32, Logical> {
        self.rec.loc - self.window.geometry().loc
    }
}
pub struct Workspace {
    windows: Vec<Rc<RefCell<HoloWindow>>>,
    outputs: Vec<Output>,
}

impl Workspace {
    pub fn new() -> Self {
        Workspace {
            windows: Vec::new(),
            outputs: Vec::new(),
        }
    }

    pub fn windows(&self) -> impl Iterator<Item = Ref<'_, Window>> {
        self.windows
            .iter()
            .map(|w| Ref::map(w.borrow(), |hw| &hw.window))
    }

    pub fn holowindows(&self) -> impl Iterator<Item = Ref<'_, HoloWindow>> {
        self.windows.iter().map(|w| Ref::map(w.borrow(), |hw| hw))
    }

    pub fn add_window(&mut self, window: Rc<RefCell<HoloWindow>>) {
        // add window to vec and remap if exists
        self.windows
            .retain(|w| &w.borrow().window != &window.borrow().window);
        self.windows.push(window.clone());
    }

    pub fn remove_window(&mut self, window: &Window) -> Option<Rc<RefCell<HoloWindow>>> {
        let mut removed = None;
        self.windows.retain(|w| {
            if &w.borrow().window == window {
                removed = Some(w.clone());
                false
            } else {
                true
            }
        });
        removed
    }

    pub fn render_elements<'a, R: Renderer + ImportAll, C: From<WaylandSurfaceRenderElement<R>>>(
        &self,
        renderer: &mut R,
    ) -> Vec<C>
    where
        <R as Renderer>::TextureId: Texture + 'static,
    {
        let mut render_elements: Vec<C> = Vec::new();
        for element in &self.windows {
            render_elements.append(&mut element.borrow().window.render_elements(
                renderer,
                element.borrow().render_location().to_physical(1),
                Scale::from(1.0),
            ));
        }
        render_elements
    }

    pub fn outputs(&self) -> impl Iterator<Item = &Output> {
        self.outputs.iter()
    }

    pub fn add_output(&mut self, output: Output) {
        self.outputs.push(output);
    }

    pub fn _remove_output(&mut self, output: &Output) {
        self.outputs.retain(|o| o != output);
    }

    pub fn output_geometry(&self, o: &Output) -> Option<Rectangle<i32, Logical>> {
        if !self.outputs.contains(o) {
            return None;
        }

        let transform: Transform = o.current_transform();
        o.current_mode().map(|mode| {
            Rectangle::from_loc_and_size(
                (0, 0),
                transform
                    .transform_size(mode.size)
                    .to_f64()
                    .to_logical(o.current_scale().fractional_scale())
                    .to_i32_ceil(),
            )
        })
    }

    pub fn window_under<P: Into<Point<f64, Logical>>>(
        &self,
        point: P,
    ) -> Option<(Ref<'_, Window>, Point<i32, Logical>)> {
        let point = point.into();
        self.windows
            .iter()
            .filter(|e| e.borrow().bbox().to_f64().contains(point))
            .find_map(|e| {
                // we need to offset the point to the location where the surface is actually drawn
                let render_location = e.borrow().render_location();
                if e.borrow()
                    .window
                    .is_in_input_region(&(point - render_location.to_f64()))
                {
                    Some((Ref::map(e.borrow(), |hw| &hw.window), render_location))
                } else {
                    None
                }
            })
    }

    pub fn contains_window(&self, window: &Window) -> bool {
        self.windows.iter().any(|w| &w.borrow().window == window)
    }
}
