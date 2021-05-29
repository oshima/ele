use self::Event::*;
use crate::coord::Pos;

pub enum Event {
    Insert(Pos, String, usize),
    InsertMv(Pos, String, usize),
    Remove(Pos, Pos, usize),
    RemoveMv(Pos, Pos, usize),
    Indent(Pos, String, usize),
}

impl Event {
    pub fn merge(self, other: Self) -> Option<Self> {
        match (self, other) {
            (Insert(pos, str1, id), Insert(_, str2, _)) => {
                Some(Insert(pos, str1 + &str2, id))
            }
            (InsertMv(_, str1, id), InsertMv(pos, str2, _)) => {
                Some(InsertMv(pos, str2 + &str1, id))
            }
            (Remove(pos1, _, id), Remove(_, pos2, _)) => {
                Some(Remove(pos1, pos2, id))
            }
            (RemoveMv(pos1, _, id), RemoveMv(_, pos2, _)) => {
                Some(RemoveMv(pos1, pos2, id))
            }
            _ => None,
        }
    }

    pub fn id(&self) -> usize {
        match self {
            Insert(_, _, id) => *id,
            InsertMv(_, _, id) => *id,
            Remove(_, _, id) => *id,
            RemoveMv(_, _, id) => *id,
            Indent(_, _, id) => *id,
        }
    }
}
