use crate::coord::Pos;

pub enum Event {
    Insert(Pos, String, bool),
    Delete(Pos, Pos, bool),
    Indent(usize, String),
    Unindent(usize, usize),
}
