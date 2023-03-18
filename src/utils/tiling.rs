use smithay::utils::{Logical, Point, Rectangle, Size};

use super::workspace::Workspace;

pub fn bsp_layout(workspace: &Workspace) -> Vec<Rectangle<i32, Logical>> {
    // let output = workspace.outputs().next().unwrap().current_mode().unwrap().size;
    let output: Size<i32, Logical> = Size::from((1920, 1080));
    let mut current_geometry: Rectangle<i32, Logical> = Rectangle {
        loc: Point::from((0, 0)),
        size: Size::from((output.w, output.h)),
    };
    let mut layout: Vec<Rectangle<i32, Logical>> = Vec::new();
    let noofwindows = workspace.windows().count();
    let mut tileside = false;
    for i in 0..noofwindows {
        let loc;
        if tileside {
            loc = Point::from((output.w - current_geometry.size.w, current_geometry.loc.y))
        } else {
            loc = Point::from((current_geometry.loc.x, output.h - current_geometry.size.h))
        }
        tileside = !tileside;
        if noofwindows > i + 1 {
            let size;
            if tileside {
                size = Size::from((current_geometry.size.w / 2, current_geometry.size.h));
            } else {
                size = Size::from((current_geometry.size.w, current_geometry.size.h / 2));
            }
            current_geometry = Rectangle {
                loc: loc,
                size: size,
            };
        } else {
            current_geometry = Rectangle {
                loc: loc,
                size: current_geometry.size,
            };
        }
        layout.push(current_geometry)
    }
    layout
}
