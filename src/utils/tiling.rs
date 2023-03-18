use smithay::{
    desktop::{Space, Window},
    utils::{Logical, Point, Rectangle, Size},
};

pub fn bsp_layout(space: &Space<Window>) -> Vec<Rectangle<i32, Logical>> {
    let mut current_geometry: Rectangle<i32, Logical> = Rectangle {
        loc: Point::from((0, 0)),
        size: Size::from((1920, 1080)),
    };
    let mut layout: Vec<Rectangle<i32, Logical>> = Vec::new();
    let noofwindows = space.elements().count();
    let mut tileside = false;
    for i in 0..noofwindows {
        let loc;
        if tileside {
            loc = Point::from((1920 - current_geometry.size.w, current_geometry.loc.y))
        } else {
            loc = Point::from((current_geometry.loc.x, 1080 - current_geometry.size.h))
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
