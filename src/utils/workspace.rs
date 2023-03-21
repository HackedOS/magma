use smithay::{
    backend::renderer::{
        element::{surface::WaylandSurfaceRenderElement, AsRenderElements},
        gles2::Gles2Renderer,
    },
    desktop::{space::SpaceElement, Window},
    output::Output,
    utils::{Logical, Point, Rectangle, Scale, Size, Transform},
};

pub struct HoloWindow {
    window: Window,
    rectangle: Rectangle<i32, Logical>,
}
impl HoloWindow {
    fn bbox(&self) -> Rectangle<i32, Logical> {
        let mut bbox = self.window.bbox();
        bbox.loc += self.rectangle.loc - self.window.geometry().loc;
        bbox
    }

    fn render_location(&self) -> Point<i32, Logical> {
        self.rectangle.loc - self.window.geometry().loc
    }
}
pub struct Workspace {
    windows: Vec<HoloWindow>,
    outputs: Vec<Output>,
    id: u8,
}

impl Workspace {
    pub fn new(id: u8) -> Self {
        Workspace {
            windows: Vec::new(),
            outputs: Vec::new(),
            id,
        }
    }

    pub fn windows(&self) -> impl Iterator<Item = &Window> {
        self.windows.iter().map(|w| &w.window)
    }

    pub fn add_window(&mut self, window: Window, geometry: Rectangle<i32, Logical>) {
        // add window to vec and remap if exists
        if let Some(w) = self.windows.iter_mut().find(|w| w.window == window) {
            w.rectangle = geometry;
        } else {
            self.windows.push(HoloWindow {
                window,
                rectangle: geometry,
            });
        }
    }

    pub fn remove_window(&mut self, window: &Window) -> usize {
        // remove window from vec and return index
        let index = self
            .windows
            .iter()
            .position(|w| &w.window == window)
            .unwrap();
        self.windows.remove(index);
        index
    }

    pub fn geometry_from_index(&self, index: usize) -> Option<Rectangle<i32, Logical>> {
        //return rectangle of given window
        if let Some(w) = self.windows.get(index) {
            Some(w.rectangle)
        } else {
            None
        }
    }

    pub fn geometry(&self, window: &Window) -> Rectangle<i32, Logical> {
        //return rectangle of given window
        self.windows
            .iter()
            .find(|w| &w.window == window)
            .unwrap()
            .rectangle
    }

    pub fn window_from_index(&self, index: usize) -> Option<&Window> {
        //return window from index
        if let Some(w) = self.windows.get(index) {
            Some(&w.window)
        } else {
            None
        }
    }

    pub fn last_geometry(&self) -> Option<Rectangle<i32, Logical>> {
        if let Some(w) = self.windows.last() {
            Some(w.rectangle)
        } else {
            None
        }
    }

    pub fn render_elements(
        &self,
        renderer: &mut Gles2Renderer,
    ) -> Vec<WaylandSurfaceRenderElement<Gles2Renderer>> {
        let mut render_elements: Vec<WaylandSurfaceRenderElement<Gles2Renderer>> = Vec::new();
        for element in &self.windows {
            render_elements.append(&mut element.window.render_elements(
                renderer,
                (element.rectangle.loc.x, element.rectangle.loc.y).into(),
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
    ) -> Option<(&Window, Point<i32, Logical>)> {
        let point = point.into();
        self.windows
            .iter()
            .filter(|e| e.bbox().to_f64().contains(point))
            .find_map(|e| {
                // we need to offset the point to the location where the surface is actually drawn
                let render_location = e.render_location();
                if e.window
                    .is_in_input_region(&(point - render_location.to_f64()))
                {
                    Some((&e.window, render_location))
                } else {
                    None
                }
            })
    }

    pub fn contains_window(&self, window: &Window) -> bool {
        self.windows.iter().any(|w| &w.window == window)
    }
}

pub struct Workspaces {
    workspaces: Vec<Workspace>,
    current: u8,
}

impl Workspaces {
    pub fn new(workspaceamount: u8) -> Self {
        Workspaces {
            workspaces: (0..workspaceamount).map(|id| Workspace::new(id)).collect(),
            current: 0,
        }
    }

    pub fn iter(&mut self) -> impl Iterator<Item = &mut Workspace> {
        self.workspaces.iter_mut()
    }

    pub fn current(&mut self) -> &mut Workspace {
        &mut self.workspaces[self.current as usize]
    }

    pub fn all_windows(&self) -> impl Iterator<Item = &Window> {
        self.workspaces.iter().flat_map(|w| w.windows())
    }

    pub fn workspace_from_window(&mut self, window: &Window) -> Option<&mut Workspace> {
        self.workspaces
            .iter_mut()
            .find(|w| w.contains_window(window))
    }

    pub fn activate(&mut self, id: u8) {
        self.current = id;
    }
}
