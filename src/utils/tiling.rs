use std::{cell::RefCell, rc::Rc};

use smithay::{
    desktop::Window,
    utils::{Point, Rectangle, Size},
};

use super::{
    binarytree::{HorizontalOrVertical, TiledHoloWindow},
    workspace::{HoloWindow, Workspace},
};

pub enum WindowLayoutEvent {
    Added,
    Removed,
    Resized,
}

pub fn bsp_layout(workspace: &mut Workspace, window: Window, event: WindowLayoutEvent) {
    let output = workspace
        .outputs()
        .next()
        .unwrap()
        .current_mode()
        .unwrap()
        .size;

    match event {
        WindowLayoutEvent::Added => {
            let tiledwindow;
            if let Some(d) = workspace.layout_tree.last() {
                let size;
                let split;
                match d.split {
                    HorizontalOrVertical::Horizontal => {
                        size = Size::from((
                            d.element.borrow().rec.size.w / 2,
                            d.element.borrow().rec.size.h,
                        ));

                        split = HorizontalOrVertical::Vertical;
                    }
                    HorizontalOrVertical::Vertical => {
                        size = Size::from((
                            d.element.borrow().rec.size.w,
                            d.element.borrow().rec.size.h / 2,
                        ));
                        split = HorizontalOrVertical::Horizontal;
                    }
                }

                d.element.borrow_mut().rec.size = size;

                let loc;
                match d.split {
                    HorizontalOrVertical::Horizontal => {
                        loc = Point::from((
                            output.w - d.element.borrow().rec.size.w,
                            d.element.borrow().rec.loc.y,
                        ));
                    }
                    HorizontalOrVertical::Vertical => {
                        loc = Point::from((
                            d.element.borrow().rec.loc.x,
                            output.h - d.element.borrow().rec.size.h,
                        ));
                    }
                }

                tiledwindow = TiledHoloWindow {
                    element: Rc::new(RefCell::new(HoloWindow {
                        window,
                        rec: Rectangle { loc, size },
                    })),
                    split,
                    ratio: 0.5,
                };
            } else {
                tiledwindow = TiledHoloWindow {
                    element: Rc::new(RefCell::new(HoloWindow {
                        window,
                        rec: Rectangle {
                            loc: Point::from((0, 0)),
                            size: Size::from((output.w, output.h)),
                        },
                    })),
                    split: HorizontalOrVertical::Horizontal,
                    ratio: 0.5,
                };
            }

            workspace.layout_tree.insert(tiledwindow.clone());

            workspace.add_window(tiledwindow.element);
        }
        WindowLayoutEvent::Removed => {
            workspace.layout_tree.remove(&window);
        }
        WindowLayoutEvent::Resized => todo!(),
    }
}
