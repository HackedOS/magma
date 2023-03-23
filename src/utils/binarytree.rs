use std::{cell::RefCell, rc::Rc};

use smithay::desktop::Window;

use super::workspace::HoloWindow;

#[derive(Debug, Clone)]
pub enum BinaryTree {
    Empty,
    Window(TiledHoloWindow),
    Split {
        left: Box<BinaryTree>,
        right: Box<BinaryTree>,
    },
}

#[derive(Debug, Clone)]
pub struct TiledHoloWindow {
    pub element: Rc<RefCell<HoloWindow>>,
    pub split: HorizontalOrVertical,
    pub ratio: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum HorizontalOrVertical {
    Horizontal,
    Vertical,
}

impl BinaryTree {
    pub fn new() -> Self {
        BinaryTree::Empty
    }

    pub fn insert(&mut self, window: TiledHoloWindow) {
        match self {
            BinaryTree::Empty => {
                *self = BinaryTree::Window(window);
            }
            BinaryTree::Window(w) => {
                *self = BinaryTree::Split {
                    left: Box::new(BinaryTree::Window(w.clone())),
                    right: Box::new(BinaryTree::Window(window)),
                };
            }
            BinaryTree::Split { left: _, right } => {
                right.insert(window);
            }
        }
    }

    pub fn remove(&mut self, window: &Window) {
        match self {
            BinaryTree::Empty => {}
            BinaryTree::Window(w) => {
                // Should only happen if this is the root
                if w.element.borrow().window == *window {
                    *self = BinaryTree::Empty;
                }
            }
            BinaryTree::Split { left, right } => {
                if let BinaryTree::Window(w) = left.as_ref() {
                    if w.element.borrow().window == *window {
                        *self = *right.clone();
                        return;
                    }
                }
                if let BinaryTree::Window(w) = right.as_ref() {
                    if w.element.borrow().window == *window {
                        *self = *left.clone();
                        return;
                    }
                }
                left.remove(window);
                right.remove(window);
            }
        }
    }

    pub fn last(&self) -> Option<TiledHoloWindow> {
        match self {
            BinaryTree::Empty => None,
            BinaryTree::Window(w) => Some(w.clone()),
            BinaryTree::Split { left, right } => right.last().or(left.last()),
        }
    }
}
