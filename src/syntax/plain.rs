use crate::row::Row;
use crate::syntax::{Hl, Syntax};

pub struct Plain;

impl Syntax for Plain {
    fn name(&self) -> &'static str {
        "Plain"
    }

    fn highlight(&self, row: &mut Row) {
        row.hls.clear();
        row.hls.resize(row.render.len(), Hl::Default);
    }
}
