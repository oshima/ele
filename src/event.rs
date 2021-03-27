use self::Event::*;
use crate::coord::Pos;

pub enum Event {
    Insert(Pos, String, bool),
    Delete(Pos, Pos, bool),
    Indent(Pos, String),
    Unindent(Pos, usize),
}

impl Event {
    pub fn merge(self, other: Self) -> Option<Self> {
        match self {
            Insert(pos1, string1, mv) => match other {
                Insert(pos2, string2, _) => {
                    if mv {
                        Some(Insert(pos2, string2 + &string1, mv))
                    } else {
                        Some(Insert(pos1, string1 + &string2, mv))
                    }
                },
                _ => None,
            },
            Delete(pos1, _, mv) => match other {
                Delete(_, pos2, _) => Some(Delete(pos1, pos2, mv)),
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
