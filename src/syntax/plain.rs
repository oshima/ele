use crate::row::Row;
use crate::syntax::{Hl, Syntax};

pub struct Plain;

impl Syntax for Plain {
    fn name(&self) -> &'static str {
        "Plain"
    }

    fn highlight(&self, rows: &mut [Row]) {
        for (i, row) in rows.iter_mut().enumerate() {
            if i > 0 && row.hl_context.is_some() {
                break;
            }
            row.hl_context = Some(String::new());
            row.hls.clear();
            row.hls.resize(row.string.len(), Hl::Default);
        }
    }
}
