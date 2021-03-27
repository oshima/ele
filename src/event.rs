use self::Event::*;
use crate::coord::Pos;

pub enum Event {
    Insert(Pos, String),
    InsertMv(Pos, String),
    Remove(Pos, Pos),
    RemoveMv(Pos, Pos),
    Indent(Pos, String),
    Unindent(Pos, usize),
}

impl Event {
    pub fn merge(self, other: Self) -> Option<Self> {
        match self {
            Insert(pos, string1) => match other {
                Insert(_, string2) => Some(Insert(pos, string1 + &string2)),
                _ => None,
            },
            InsertMv(_, string1) => match other {
                InsertMv(pos, string2) => Some(InsertMv(pos, string2 + &string1)),
                _ => None,
            },
            Remove(pos1, _) => match other {
                Remove(_, pos2) => Some(Remove(pos1, pos2)),
                _ => None,
            },
            RemoveMv(pos1, _) => match other {
                RemoveMv(_, pos2) => Some(RemoveMv(pos1, pos2)),
                _ => None,
            },
            Indent(pos, string1) => match other {
                Indent(_, string2) => Some(Indent(pos, string2 + &string1)),
                _ => None,
            },
            Unindent(_, width1) => match other {
                Unindent(pos, width2) => Some(Unindent(pos, width1 + width2)),
                _ => None,
            }
        }
    }
}
