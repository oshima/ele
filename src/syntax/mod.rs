mod plain;
mod rust;

use crate::canvas::Term;
use crate::row::Row;
use crate::syntax::plain::Plain;
use crate::syntax::rust::Rust;

pub trait Syntax {
    fn name(&self) -> &'static str;
    fn fg_color(&self, term: Term) -> &'static [u8];
    fn bg_color(&self, term: Term) -> &'static [u8];
    fn indent_unit(&self) -> Option<&'static str>;
    fn update_rows(&self, rows: &mut [Row]) -> usize;
}

impl dyn Syntax {
    pub fn detect(filename: Option<&str>) -> Box<dyn Syntax> {
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
