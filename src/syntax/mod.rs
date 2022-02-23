mod plain;
mod ruby;
mod rust;

use std::path::Path;

use crate::canvas::Term;
use crate::row::Row;
use crate::syntax::plain::Plain;
use crate::syntax::ruby::Ruby;
use crate::syntax::rust::Rust;

pub trait Syntax {
    fn matches(file_name: &str) -> bool
    where
        Self: Sized;
    fn name(&self) -> &'static str;
    fn fg_color(&self, term: Term) -> &'static [u8];
    fn bg_color(&self, term: Term) -> &'static [u8];
    fn indent_unit(&self) -> Option<&'static str>;
    fn update_rows(&self, rows: &mut [Row]) -> usize;
}

impl dyn Syntax {
    pub fn detect(file_path: Option<&str>) -> Box<dyn Syntax> {
        let file_name = file_path
            .map_or(None, |s| Path::new(s).file_name())
            .map_or(None, |s| s.to_str());

        if let Some(file_name) = file_name {
            if Ruby::matches(file_name) {
                Box::new(Ruby)
            } else if Rust::matches(file_name) {
                Box::new(Rust)
            } else {
                Box::new(Plain)
            }
        } else {
            Box::new(Plain)
        }
    }
}
