pub mod plain;
pub mod rust;

use crate::row::Row;
use crate::syntax::plain::Plain;
use crate::syntax::rust::Rust;

pub trait Syntax {
    fn name(&self) -> &'static str;
    fn highlight(&self, row: &mut Row);
}

impl dyn Syntax {
    pub fn detect(filename: &Option<String>) -> Box<dyn Syntax> {
        if let Some(s) = filename {
            if s.ends_with(".rs") {
                Box::new(Rust)
            } else {
                Box::new(Plain)
            }
        } else {
            Box::new(Plain)
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Hl {
    Default,
    Keyword,
    Type,
    Module,
    Variable,
    Function,
    Macro,
    String,
    Comment,
}
