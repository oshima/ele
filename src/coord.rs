use std::cmp::Ordering;

#[derive(Clone, Copy, Default, PartialEq)]
pub struct Pos {
    pub x: usize,
    pub y: usize,
}

impl Pos {
    pub fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }
}

impl PartialOrd for Pos {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.y == other.y {
            self.x.partial_cmp(&other.x)
        } else {
            self.y.partial_cmp(&other.y)
        }
    }
}

pub struct Size {
    pub w: usize,
    pub h: usize,
}

impl Size {
    pub fn new(w: usize, h: usize) -> Self {
        Self { w, h }
    }
}

#[derive(Clone, Copy, Default)]
pub struct Cursor {
    pub x: usize,
    pub y: usize,
    pub last_x: usize,
}

impl Cursor {
    pub fn new(x: usize, y: usize) -> Self {
        Self { x, y, last_x: x }
    }

    pub fn as_pos(&self) -> Pos {
        Pos {
            x: self.x,
            y: self.y,
        }
    }
}
