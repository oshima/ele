use crate::coord::Pos;

pub enum Event {
    Insert(usize, Pos, String),
    InsertMv(usize, Pos, String),
    Remove(usize, Pos, Pos),
    RemoveMv(usize, Pos, Pos),
    Indent(usize, Pos, String),
}

impl Event {
    pub fn merge(self, other: Self) -> Option<Self> {
        match (self, other) {
            (Self::Insert(id, pos, str1), Self::Insert(_, _, str2)) => {
                Some(Self::Insert(id, pos, str1 + &str2))
            }
            (Self::Insert(id, _, str1), Self::InsertMv(_, pos, str2)) => {
                Some(Self::InsertMv(id, pos, str2 + &str1))
            }
            (Self::InsertMv(id, _, str1), Self::InsertMv(_, pos, str2)) => {
                Some(Self::InsertMv(id, pos, str2 + &str1))
            }
            (Self::InsertMv(id, pos, str1), Self::Insert(_, _, str2)) => {
                Some(Self::Insert(id, pos, str1 + &str2))
            }
            (Self::Remove(id, pos1, _), Self::Remove(_, _, pos2)) => {
                Some(Self::Remove(id, pos1, pos2))
            }
            (Self::RemoveMv(id, pos1, _), Self::RemoveMv(_, _, pos2)) => {
                Some(Self::RemoveMv(id, pos1, pos2))
            }
            _ => None,
        }
    }

    pub fn id(&self) -> usize {
        match self {
            Self::Insert(id, ..) => *id,
            Self::InsertMv(id, ..) => *id,
            Self::Remove(id, ..) => *id,
            Self::RemoveMv(id, ..) => *id,
            Self::Indent(id, ..) => *id,
        }
    }
}
