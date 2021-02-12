use crate::canvas::Term;
use crate::face::{Bg, Fg};
use crate::row::{HlContext, Row};
use crate::syntax::{Indent, Syntax};

const UNCHECKED: HlContext = 0;
const DEFAULT: HlContext = 1;

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

    fn indent(&self) -> Indent {
        Indent::None
    }

    fn highlight(&self, rows: &mut [Row]) -> usize {
        let mut len = 0;

        for (i, row) in rows.iter_mut().enumerate() {
            if i > 0 && row.hl_context != UNCHECKED {
                break;
            }
            row.hl_context = DEFAULT;
            row.faces.clear();
            row.faces
                .resize(row.string.len(), (Fg::Default, Bg::Default));
            row.trailing_bg = Bg::Default;
            len += 1;
        }

        len
    }
}
