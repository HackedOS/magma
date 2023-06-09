use std::{cell::RefCell, rc::Rc};

use smithay::{
    desktop::{Window, layer_map_for_output},
    utils::{Logical, Physical, Point, Rectangle, Size},
};

use super::{
    binarytree::{BinaryTree, HorizontalOrVertical},
    workspaces::{MagmaWindow, Workspace},
};

pub enum WindowLayoutEvent {
    Added,
    Removed,
}

pub fn bsp_layout(
    workspace: &mut Workspace,
    window: Window,
    event: WindowLayoutEvent,
    gaps: (i32, i32),
) {
    let output = layer_map_for_output(workspace
        .outputs()
        .next()
        .unwrap()).non_exclusive_zone();

    match event {
        WindowLayoutEvent::Added => {
            let window = Rc::new(RefCell::new(MagmaWindow {
                window,
                rec: Rectangle {
                    loc: Point::from((gaps.0 + output.loc.x, gaps.0 + output.loc.y)),
                    size: Size::from((output.size.w - (gaps.0 * 2), output.size.h - (gaps.0 * 2))),
                },
            }));
            workspace.add_window(window);

            bsp_update_layout(workspace, gaps);
        }
        WindowLayoutEvent::Removed => {
            workspace.remove_window(&window);
            bsp_update_layout(workspace, gaps);
        }
    }
    dbg!(workspace.layout_tree.clone());
}

pub fn bsp_update_layout(workspace: &mut Workspace, gaps: (i32, i32)) {
    //recalculate the size and location of the windows

    let output = layer_map_for_output(workspace
        .outputs()
        .next()
        .unwrap()).non_exclusive_zone();

    let output_full = workspace.outputs().next().unwrap().current_mode().unwrap().size;

    match &mut workspace.layout_tree {
        BinaryTree::Empty => {}
        BinaryTree::Window(w) => {
            w.borrow_mut().rec = Rectangle {
                loc: Point::from((gaps.0 + gaps.1 + output.loc.x, gaps.0 + gaps.1 + output.loc.y)),
                size: Size::from((
                    output.size.w - ((gaps.0 + gaps.1) * 2),
                    output.size.h - ((gaps.0 + gaps.1) * 2),
                )),
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
                        loc: Point::from((gaps.0 + output.loc.x, gaps.0 + output.loc.y)),
                        size: Size::from((output.size.w - (gaps.0 * 2), output.size.h - (gaps.0 * 2))),
                    },
                    *split,
                    *ratio,
                    Size::from((output_full.w - gaps.0, output_full.h - gaps.0)),
                    gaps,
                )
            }
        }
    }
    for magmawindow in workspace.magmawindows() {
        let xdg_toplevel = magmawindow.window.toplevel();
        xdg_toplevel.with_pending_state(|state| {
            state.size = Some(magmawindow.rec.size);
        });
        xdg_toplevel.send_configure();
    }
}

pub fn generate_layout(
    tree: &mut BinaryTree,
    lastwin: &Rc<RefCell<MagmaWindow>>,
    lastgeo: Rectangle<i32, Logical>,
    split: HorizontalOrVertical,
    ratio: f32,
    output: Size<i32, Physical>,
    gaps: (i32, i32),
) {
    let size;
    match split {
        HorizontalOrVertical::Horizontal => {
            size = Size::from(((lastgeo.size.w as f32 * ratio) as i32, lastgeo.size.h));
        }
        HorizontalOrVertical::Vertical => {
            size = Size::from((lastgeo.size.w, (lastgeo.size.h as f32 * ratio) as i32));
        }
    }

    let loc: Point<i32, Logical>;
    match split {
        HorizontalOrVertical::Horizontal => {
            loc = Point::from((lastgeo.loc.x, output.h - size.h));
        }
        HorizontalOrVertical::Vertical => {
            loc = Point::from((output.w - size.w, lastgeo.loc.y));
        }
    }

    let recgapped = Rectangle {
        size: Size::from((size.w - (gaps.1 * 2), (size.h - (gaps.1 * 2)))),
        loc: Point::from((loc.x + gaps.1, loc.y + gaps.1)),
    };

    lastwin.borrow_mut().rec = recgapped;

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
    let recgapped = Rectangle {
        size: Size::from((size.w - (gaps.1 * 2), (size.h - (gaps.1 * 2)))),
        loc: Point::from((loc.x + gaps.1, loc.y + gaps.1)),
    };
    match tree {
        BinaryTree::Empty => {}
        BinaryTree::Window(w) => w.borrow_mut().rec = recgapped,
        BinaryTree::Split {
            split,
            ratio,
            left,
            right,
        } => {
            if let BinaryTree::Window(w) = left.as_mut() {
                w.borrow_mut().rec = rec;
                generate_layout(right.as_mut(), &w, rec, *split, *ratio, output, gaps)
            }
        }
    }
}
