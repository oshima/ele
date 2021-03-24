use crate::coord::Pos;

pub enum Event {
    Insert(Pos, String, bool),
    Delete(Pos, Pos, bool),
    Indent(Pos, String),
    Unindent(Pos, usize),
}
