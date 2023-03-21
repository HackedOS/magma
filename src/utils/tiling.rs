use smithay::{
    desktop::Window,
    utils::{Logical, Point, Rectangle, Size},
};

use super::workspace::Workspace;

pub enum WindowLayoutEvent {
    Added,
    Removed,
    Resized,
}

pub fn bsp_layout(workspace: &mut Workspace, event: WindowLayoutEvent, window: Window) {
    let output = workspace
        .outputs()
        .next()
        .unwrap()
        .current_mode()
        .unwrap()
        .size;

    match event {
        WindowLayoutEvent::Added => {
            let tileside = if workspace.windows().count() % 2 == 0 {
                true
            } else {
                false
            };
            let mut geometry;
            if let Some(rec) = workspace.last_geometry() {
                if tileside {
                    let loc = Point::from((output.w - rec.size.w, rec.loc.y));
                    geometry = Rectangle {
                        size: Size::from((rec.size.w, rec.size.h / 2)),
                        loc: loc,
                    };
                } else {
                    let loc = Point::from((rec.loc.x, output.h - rec.size.h));
                    geometry = Rectangle {
                        size: Size::from((rec.size.w / 2, rec.size.h)),
                        loc: loc,
                    };
                }
            } else {
                geometry = Rectangle {
                    loc: Point::from((0, 0)),
                    size: output.to_logical(1),
                };
            }
            if let Some(last) = workspace.windows().last() {
                workspace.add_window(last.clone(), geometry)
            }
            if let Some(rec) = workspace.last_geometry() {
                if !tileside {
                    geometry.loc = Point::from((output.w - rec.size.w, rec.loc.y));
                } else {
                    geometry.loc = Point::from((rec.loc.x, output.h - rec.size.h));
                }
            }
            workspace.add_window(window, geometry);
        }
        WindowLayoutEvent::Removed => {
            workspace.remove_window(&window);
        }
        WindowLayoutEvent::Resized => todo!(),
    }
}
