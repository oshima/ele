use crate::coord::Pos;

pub enum Event {
    Insert(Pos, String, bool),
    Delete(Pos, String, bool),
}

impl Event {
    pub fn reverse(self) -> Self {
        match self {
            Self::Insert(pos, string, mv) => Self::Delete(pos, string, mv),
            Self::Delete(pos, string, mv) => Self::Insert(pos, string, mv),
        }
    }
}
