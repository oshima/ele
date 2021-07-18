use crate::coord::Pos;

pub enum Event {
    Insert(usize, Pos, String, bool),
    Remove(usize, Pos, Pos, bool),
    Indent(usize, Pos, String),
}

impl Event {
    pub fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Insert(id, pos, str1, _), Self::Insert(_, _, str2, false)) => {
                Self::Insert(id, pos, str1 + &str2, false)
            }
            (Self::Insert(id, _, str1, _), Self::Insert(_, pos, str2, true)) => {
                Self::Insert(id, pos, str2 + &str1, true)
            }
            (Self::Remove(id, pos1, _, false), Self::Remove(_, _, pos2, false)) => {
                Self::Remove(id, pos1, pos2, false)
            }
            (Self::Remove(id, pos1, _, true), Self::Remove(_, _, pos2, true)) => {
                Self::Remove(id, pos1, pos2, true)
            }
            _ => panic!(),
        }
    }

    pub fn id(&self) -> usize {
        match self {
            Self::Insert(id, ..) | Self::Remove(id, ..) | Self::Indent(id, ..) => *id,
        }
    }
}
