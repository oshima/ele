use self::Event::*;
use crate::coord::Pos;

pub enum Event {
    Insert(usize, Pos, String, bool),
    Remove(usize, Pos, Pos, bool),
    Indent(usize, Pos, String),
}

impl Event {
    #[rustfmt::skip]
    pub fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Insert(id, pos1, str1, false), Insert(_, _, str2, false)) => {
                Insert(id, pos1, str1 + &str2, false)
            }
            (Insert(id, _, str1, true), Insert(_, pos2, str2, true)) => {
                Insert(id, pos2, str2 + &str1, true)
            }
            (Remove(id, pos1, _, true), Remove(_, _, pos2, true)) => {
                Remove(id, pos1, pos2, true)
            }
            _ => panic!(),
        }
    }

    pub fn id(&self) -> usize {
        match self {
            Insert(id, ..) | Remove(id, ..) | Indent(id, ..) => *id,
        }
    }
}
