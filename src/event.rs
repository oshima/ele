use crate::coord::Pos;

pub enum Event {
    Insert(Pos, String, u64),
    InsertMv(Pos, String, u64),
    Remove(Pos, Pos, u64),
    RemoveMv(Pos, Pos, u64),
    Indent(Pos, String, u64),
}

impl Event {
    pub fn merge(self, other: Self) -> Option<Self> {
        match (self, other) {
            (Self::Insert(pos, str1, id), Self::Insert(_, str2, _)) => {
                Some(Self::Insert(pos, str1 + &str2, id))
            }
            (Self::InsertMv(_, str1, id), Self::InsertMv(pos, str2, _)) => {
                Some(Self::InsertMv(pos, str2 + &str1, id))
            }
            (Self::Remove(pos1, _, id), Self::Remove(_, pos2, _)) => {
                Some(Self::Remove(pos1, pos2, id))
            }
            (Self::RemoveMv(pos1, _, id), Self::RemoveMv(_, pos2, _)) => {
                Some(Self::RemoveMv(pos1, pos2, id))
            }
            _ => None,
        }
    }

    pub fn id(&self) -> u64 {
        match self {
            Self::Insert(_, _, id) => *id,
            Self::InsertMv(_, _, id) => *id,
            Self::Remove(_, _, id) => *id,
            Self::RemoveMv(_, _, id) => *id,
            Self::Indent(_, _, id) => *id,
        }
    }
}
