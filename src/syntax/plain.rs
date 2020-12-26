use crate::canvas::Term;
use crate::face::Face;
use crate::row::{HlContext, Row};
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
        let mut len = 0;

        for (i, row) in rows.iter_mut().enumerate() {
            if i > 0 && row.hl_context != UNDEFINED {
                break;
            }
            row.hl_context = NORMAL;
            row.faces.clear();
            row.faces.resize(row.string.len(), Face::Default);
            len += 1;
        }

        len
    }
}
