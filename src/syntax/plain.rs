use crate::canvas::Term;
use crate::face::{Bg, Fg};
use crate::row::Row;
use crate::syntax::Syntax;

pub struct Plain;

impl Syntax for Plain {
    fn name(&self) -> &'static str {
        "Plain"
    }

    fn fg_color(&self, term: Term) -> &'static [u8] {
        match term {
            Term::TrueColor => fg_color!(0, 0, 0),
            Term::Color256 => fg_color256!(16),
            Term::Color16 => fg_color16!(black),
        }
    }

    fn bg_color(&self, term: Term) -> &'static [u8] {
        match term {
            Term::TrueColor => bg_color!(255, 255, 255),
            Term::Color256 => bg_color256!(231),
            Term::Color16 => bg_color16!(white),
        }
    }

    fn indent_unit(&self) -> Option<&'static str> {
        None
    }

    fn update_rows(&self, rows: &mut [Row]) -> usize {
        for (i, row) in rows.iter_mut().enumerate() {
            if i > 0 && row.context.is_some() {
                return i;
            }
            row.context = Some(String::new());
            row.faces.clear();
            row.faces
                .resize(row.string.len(), (Fg::Default, Bg::Default));
            row.trailing_bg = Bg::Default;
        }

        rows.len()
    }
}
