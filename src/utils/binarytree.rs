use smithay::desktop::Window;
use std::fmt::Debug;
use std::{cell::RefCell, rc::Rc};

use super::workspaces::HoloWindow;

#[derive(Clone)]
pub enum BinaryTree {
    Empty,
    Window(Rc<RefCell<HoloWindow>>),
    Split {
        split: HorizontalOrVertical,
        ratio: f32,
        left: Box<BinaryTree>,
        right: Box<BinaryTree>,
    },
}

impl Debug for BinaryTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "Empty"),
            Self::Window(w) => w.borrow().rec.fmt(f),
            Self::Split {
                left,
                right,
                split,
                ratio,
            } => f
                .debug_struct("Split")
                .field("split", split)
                .field("ratio", ratio)
                .field("left", left)
                .field("right", right)
                .finish(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HorizontalOrVertical {
    Horizontal,
    Vertical,
}

impl BinaryTree {
    pub fn new() -> Self {
        BinaryTree::Empty
    }

    pub fn insert(
        &mut self,
        window: Rc<RefCell<HoloWindow>>,
        splitnew: HorizontalOrVertical,
        rationew: f32,
    ) {
        match self {
            BinaryTree::Empty => {
                *self = BinaryTree::Window(window);
            }
            BinaryTree::Window(w) => {
                *self = BinaryTree::Split {
                    left: Box::new(BinaryTree::Window(w.clone())),
                    right: Box::new(BinaryTree::Window(window)),
                    split: splitnew,
                    ratio: rationew,
                };
            }
            BinaryTree::Split {
                left: _,
                right,
                split: _,
                ratio: _,
            } => {
                right.insert(window, splitnew, rationew);
            }
        }
    }

    pub fn remove(&mut self, window: &Window) {
        match self {
            BinaryTree::Empty => {}
            BinaryTree::Window(w) => {
                // Should only happen if this is the root
                if w.borrow().window == *window {
                    *self = BinaryTree::Empty;
                }
            }
            BinaryTree::Split {
                left,
                right,
                split: _,
                ratio: _,
            } => {
                if let BinaryTree::Window(w) = left.as_ref() {
                    if w.borrow().window == *window {
                        *self = *right.clone();
                        return;
                    }
                }
                if let BinaryTree::Window(w) = right.as_ref() {
                    if w.borrow().window == *window {
                        *self = *left.clone();
                        return;
                    }
                }
                left.remove(window);
                right.remove(window);
            }
        }
    }

    pub fn next_split(&self) -> HorizontalOrVertical {
        match self {
            BinaryTree::Empty => HorizontalOrVertical::Horizontal,
            BinaryTree::Window(_w) => HorizontalOrVertical::Horizontal,
            BinaryTree::Split {
                left: _,
                right,
                split,
                ratio: _,
            } => {
                if let BinaryTree::Split {
                    left: _,
                    right: _,
                    split: _,
                    ratio: _,
                } = right.as_ref()
                {
                    right.next_split()
                } else {
                    if *split == HorizontalOrVertical::Horizontal {
                        HorizontalOrVertical::Vertical
                    } else {
                        HorizontalOrVertical::Horizontal
                    }
                }
            }
        }
    }

    pub fn last(&self) -> Option<Rc<RefCell<HoloWindow>>> {
        match self {
            BinaryTree::Empty => None,
            BinaryTree::Window(w) => Some(w.clone()),
            BinaryTree::Split {
                left,
                right,
                split,
                ratio,
            } => right.last().or(left.last()),
        }
    }
}
