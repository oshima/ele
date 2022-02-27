use self::EditKind::*;
use crate::coord::Pos;

pub struct Edit {
    pub time: usize,
    pub kind: EditKind,
}

pub enum EditKind {
    Insert(Pos, String, bool),
    Remove(Pos, Pos, bool),
    Indent(Pos, String),
}

#[rustfmt::skip]
impl Edit {
    pub fn insert(time: usize, pos: Pos, string: String, mv: bool) -> Self {
        Self { time, kind: Insert(pos, string, mv) }
    }

    pub fn remove(time: usize, pos1: Pos, pos2: Pos, mv: bool) -> Self {
        Self { time, kind: Remove(pos1, pos2, mv) }
    }

    pub fn indent(time: usize, pos: Pos, string: String) -> Self {
        Self { time, kind: Indent(pos, string) }
    }

    pub fn merge(self, other: Self) -> Self {
        let kind = match (self.kind, other.kind) {
            (Insert(pos1, str1, true), Insert(_, str2, true)) => {
                Insert(pos1, str1 + &str2, true)
            }
            (Insert(_, str1, false), Insert(pos2, str2, false)) => {
                Insert(pos2, str2 + &str1, false)
            }
            (Remove(_, pos1, true), Remove(pos2, _, true)) => {
                Remove(pos2, pos1, true)
            }
            _ => panic!(),
        };

        Self { time: self.time, kind }
    }
}
