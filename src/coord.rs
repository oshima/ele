use std::cmp::Ordering;

#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub struct Pos {
    pub x: usize,
    pub y: usize,
}

impl Pos {
    pub fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }
}

impl Ord for Pos {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.y == other.y {
            self.x.cmp(&other.x)
        } else {
            self.y.cmp(&other.y)
        }
    }
}

impl PartialOrd for Pos {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
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
}
