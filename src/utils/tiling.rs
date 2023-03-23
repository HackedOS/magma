use std::{cell::RefCell, rc::Rc};

use smithay::{
    desktop::Window,
    utils::{Logical, Physical, Point, Rectangle, Size},
};

use super::{
    binarytree::{BinaryTree, HorizontalOrVertical},
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
            let window = Rc::new(RefCell::new(HoloWindow {
                window,
                rec: Rectangle {
                    loc: Point::from((0, 0)),
                    size: Size::from((output.w, output.h)),
                },
            }));
            workspace
                .layout_tree
                .insert(window.clone(), workspace.layout_tree.next_split(), 0.5);

            bsp_update_layout(workspace);

            workspace.add_window(window)
        }
        WindowLayoutEvent::Removed => {
            workspace.layout_tree.remove(&window);
            bsp_update_layout(workspace);
        }
        WindowLayoutEvent::Resized => todo!(),
    }
    println!("{:#?}", workspace.layout_tree);
}

pub fn bsp_update_layout(workspace: &mut Workspace) {
    //recalculate the size and location of the windows

    let output = workspace
        .outputs()
        .next()
        .unwrap()
        .current_mode()
        .unwrap()
        .size;

    match &mut workspace.layout_tree {
        BinaryTree::Empty => {}
        BinaryTree::Window(w) => {
            w.borrow_mut().rec = Rectangle {
                loc: Point::from((0, 0)),
                size: Size::from((output.w, output.h)),
            };
        }
        BinaryTree::Split {
            left,
            right,
            split,
            ratio,
        } => {
            if let BinaryTree::Window(w) = left.as_mut() {
                generate_layout(
                    right.as_mut(),
                    &w,
                    Rectangle {
                        loc: Point::from((0, 0)),
                        size: output.to_logical(1),
                    },
                    *split,
                    *ratio,
                    output,
                )
            }
        }
    }
}

pub fn generate_layout(
    tree: &mut BinaryTree,
    lastwin: &Rc<RefCell<HoloWindow>>,
    lastgeo: Rectangle<i32, Logical>,
    split: HorizontalOrVertical,
    ratio: f32,
    output: Size<i32, Physical>,
) {
    let size;
    match split {
        HorizontalOrVertical::Horizontal => {
            size = Size::from((lastgeo.size.w / 2, lastgeo.size.h));
        }
        HorizontalOrVertical::Vertical => {
            size = Size::from((lastgeo.size.w, lastgeo.size.h / 2));
        }
    }

    let loc;
    match split {
        HorizontalOrVertical::Horizontal => {
            loc = Point::from((lastgeo.loc.x, output.h - size.h));
        }
        HorizontalOrVertical::Vertical => {
            loc = Point::from((output.w - size.w, lastgeo.loc.y));
        }
    }

    lastwin.borrow_mut().rec = Rectangle { size, loc };

    let loc;
    match split {
        HorizontalOrVertical::Horizontal => {
            loc = Point::from((output.w - size.w, lastgeo.loc.y));
        }
        HorizontalOrVertical::Vertical => {
            loc = Point::from((lastgeo.loc.x, output.h - size.h));
        }
    }

    let rec = Rectangle { size, loc };

    match tree {
        BinaryTree::Empty => {}
        BinaryTree::Window(w) => w.borrow_mut().rec = rec,
        BinaryTree::Split {
            split,
            ratio,
            left,
            right,
        } => {
            if let BinaryTree::Window(w) = left.as_mut() {
                w.borrow_mut().rec = rec;
                generate_layout(right.as_mut(), &w, rec, *split, *ratio, output)
            }
        }
    }
}
