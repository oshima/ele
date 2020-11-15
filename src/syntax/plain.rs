use crate::row::Row;
use crate::syntax::{Hl, HlContext, Syntax};

const UNDEFINED: HlContext = 0;
const NORMAL: HlContext = 1;

pub struct Plain;

impl Syntax for Plain {
    fn name(&self) -> &'static str {
        "Plain"
    }

    fn highlight(&self, rows: &mut [Row]) -> usize {
        let mut n_updates = 0;

        for (i, row) in rows.iter_mut().enumerate() {
            if i > 0 && row.hl_context != UNDEFINED {
                break;
            }
            row.hl_context = NORMAL;
            row.hls.clear();
            row.hls.resize(row.string.len(), Hl::Default);
            n_updates += 1;
        }

        n_updates
    }
}
