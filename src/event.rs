use self::Event::*;
use crate::coord::Pos;

pub enum Event {
    Insert(Pos, String, Option<u64>),
    InsertMv(Pos, String, Option<u64>),
    Remove(Pos, Pos, Option<u64>),
    RemoveMv(Pos, Pos, Option<u64>),
    Indent(Pos, String, Option<u64>),
    Unindent(Pos, usize, Option<u64>),
}

impl Event {
    pub fn merge(self, other: Self) -> Option<Self> {
        match (self, other) {
            (Insert(pos, str1, _), Insert(_, str2, _)) => Some(Insert(pos, str1 + &str2, None)),
            (InsertMv(_, str1, _), InsertMv(pos, str2, _)) => {
                Some(InsertMv(pos, str2 + &str1, None))
            }
            (Remove(pos1, _, _), Remove(_, pos2, _)) => Some(Remove(pos1, pos2, None)),
            (RemoveMv(pos1, _, _), RemoveMv(_, pos2, _)) => Some(RemoveMv(pos1, pos2, None)),
            (Indent(pos, str1, _), Indent(_, str2, _)) => Some(Indent(pos, str2 + &str1, None)),
            (Unindent(_, width1, _), Unindent(pos, width2, _)) => {
                Some(Unindent(pos, width1 + width2, None))
            }
            _ => None,
        }
    }

    pub fn stamp(&mut self, clock: u64) {
        let stamp = match self {
            Insert(_, _, stamp) => stamp,
            InsertMv(_, _, stamp) => stamp,
            Remove(_, _, stamp) => stamp,
            RemoveMv(_, _, stamp) => stamp,
            Indent(_, _, stamp) => stamp,
            Unindent(_, _, stamp) => stamp,
        };
        stamp.replace(clock);
    }

    pub fn get_stamp(&self) -> Option<u64> {
        match self {
            Insert(_, _, stamp) => *stamp,
            InsertMv(_, _, stamp) => *stamp,
            Remove(_, _, stamp) => *stamp,
            RemoveMv(_, _, stamp) => *stamp,
            Indent(_, _, stamp) => *stamp,
            Unindent(_, _, stamp) => *stamp,
        }
    }
}
