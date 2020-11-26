use crate::canvas::Term;
use crate::hl::{Hl, HlContext};
use crate::row::Row;
use crate::syntax::Syntax;

const UNDEFINED: HlContext = 0;
const NORMAL: HlContext = 1;

pub struct Plain;

impl Syntax for Plain {
    fn name(&self) -> &'static str {
        "Plain"
    }

    fn color(&self, term: Term) -> &'static [u8] {
        match term {
            Term::TrueColor => b"\x1b[38;2;0;0;0m\x1b[48;2;255;255;255m",
            Term::Color256 => b"\x1b[38;5;16m\x1b[48;5;231m",
            Term::Color16 => b"\x1b[30m\x1b[47m",
        }
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
