use std::env;
use std::io::{self, Write};

use crate::face::{Bg, Fg};

#[derive(Clone, Copy)]
pub enum Term {
    TrueColor,
    Color256,
    Color16,
}

pub struct Canvas {
    pub term: Term,
    bytes: Vec<u8>,
    pub fg: Option<Fg>,
    pub bg: Option<Bg>,
    fg_colors: [Vec<u8>; 12],
    bg_colors: [Vec<u8>; 5],
}

impl Write for Canvas {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bytes.write(buf)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.bytes.flush()
    }
}

impl Canvas {
    pub fn new() -> Self {
        let mut canvas = Self {
            term: Self::detect_term(),
            bytes: Vec::new(),
            fg: None,
            bg: None,
            fg_colors: Default::default(),
            bg_colors: Default::default(),
        };
        canvas.define_colors();
        canvas
    }

    fn detect_term() -> Term {
        match env::var("COLORTERM") {
            Ok(val) if val == "truecolor" => Term::TrueColor,
            _ => match env::var("TERM") {
                Ok(val) if val.contains("256color") => Term::Color256,
                _ => Term::Color16,
            },
        }
    }

    fn define_colors(&mut self) {
        // TODO: load config file

        // default: Tomorrow Night Bright
        match self.term {
            Term::TrueColor => {
                self.define_fg_color(Fg::Default, b"\x1b[38;2;234;234;234m");
                self.define_fg_color(Fg::Keyword, b"\x1b[38;2;195;151;216m");
                self.define_fg_color(Fg::Type, b"\x1b[38;2;231;197;71m");
                self.define_fg_color(Fg::Module, b"\x1b[38;2;112;192;177m");
                self.define_fg_color(Fg::Variable, b"\x1b[38;2;231;140;69m");
                self.define_fg_color(Fg::Function, b"\x1b[38;2;122;166;218m");
                self.define_fg_color(Fg::Macro, b"\x1b[38;2;112;192;177m");
                self.define_fg_color(Fg::String, b"\x1b[38;2;185;202;74m");
                self.define_fg_color(Fg::Comment, b"\x1b[38;2;150;152;150m");
                self.define_fg_color(Fg::Prompt, b"\x1b[38;2;122;166;218m");
                self.define_bg_color(Bg::Match, b"\x1b[38;2;0;0;0m");
                self.define_bg_color(Bg::CurrentMatch, b"\x1b[38;2;0;0;0m");

                self.define_bg_color(Bg::Default, b"\x1b[48;2;0;0;0m");
                self.define_bg_color(Bg::Region, b"\x1b[48;2;66;66;66m");
                self.define_bg_color(Bg::StatusBar, b"\x1b[48;2;28;28;28m");
                self.define_bg_color(Bg::Match, b"\x1b[48;2;231;197;71m");
                self.define_bg_color(Bg::CurrentMatch, b"\x1b[48;2;231;140;69m");
            }
            Term::Color256 => {
                self.define_fg_color(Fg::Default, b"\x1b[38;5;255m");
                self.define_fg_color(Fg::Keyword, b"\x1b[38;5;182m");
                self.define_fg_color(Fg::Type, b"\x1b[38;5;179m");
                self.define_fg_color(Fg::Module, b"\x1b[38;5;115m");
                self.define_fg_color(Fg::Variable, b"\x1b[38;5;173m");
                self.define_fg_color(Fg::Function, b"\x1b[38;5;110m");
                self.define_fg_color(Fg::Macro, b"\x1b[38;5;115m");
                self.define_fg_color(Fg::String, b"\x1b[38;5;143m");
                self.define_fg_color(Fg::Comment, b"\x1b[38;5;246m");
                self.define_fg_color(Fg::Prompt, b"\x1b[38;5;110m");
                self.define_fg_color(Fg::Match, b"\x1b[38;5;16m");
                self.define_fg_color(Fg::CurrentMatch, b"\x1b[38;5;16m");

                self.define_bg_color(Bg::Default, b"\x1b[48;5;16m");
                self.define_bg_color(Bg::Region, b"\x1b[48;5;238m");
                self.define_bg_color(Bg::StatusBar, b"\x1b[48;5;234m");
                self.define_bg_color(Bg::Match, b"\x1b[48;5;179m");
                self.define_bg_color(Bg::CurrentMatch, b"\x1b[48;5;173m");
            }
            Term::Color16 => {
                self.define_fg_color(Fg::Default, b"\x1b[39m");
                self.define_fg_color(Fg::Keyword, b"\x1b[35m");
                self.define_fg_color(Fg::Type, b"\x1b[33m");
                self.define_fg_color(Fg::Module, b"\x1b[36m");
                self.define_fg_color(Fg::Variable, b"\x1b[31m");
                self.define_fg_color(Fg::Function, b"\x1b[34m");
                self.define_fg_color(Fg::Macro, b"\x1b[36m");
                self.define_fg_color(Fg::String, b"\x1b[32m");
                self.define_fg_color(Fg::Comment, b"\x1b[36m");
                self.define_fg_color(Fg::Prompt, b"\x1b[34m");
                self.define_fg_color(Fg::Match, b"\x1b[30m");
                self.define_fg_color(Fg::CurrentMatch, b"\x1b[30m");

                self.define_bg_color(Bg::Default, b"\x1b[40m");
                self.define_bg_color(Bg::Region, b"\x1b[47m");
                self.define_bg_color(Bg::StatusBar, b"\x1b[40m");
                self.define_bg_color(Bg::Match, b"\x1b[43m");
                self.define_bg_color(Bg::CurrentMatch, b"\x1b[41m");
            }
        }
    }

    fn define_fg_color(&mut self, fg: Fg, color: &[u8]) {
        self.fg_colors[fg as usize].extend_from_slice(color);
    }

    fn define_bg_color(&mut self, bg: Bg, color: &[u8]) {
        self.bg_colors[bg as usize].extend_from_slice(color);
    }

    #[inline]
    pub fn set_fg_color(&mut self, fg: Fg) -> io::Result<()> {
        let prev_fg = self.fg.replace(fg);

        if self.fg != prev_fg {
            self.bytes.write(&self.fg_colors[fg as usize])?;
        }
        Ok(())
    }

    #[inline]
    pub fn set_bg_color(&mut self, bg: Bg) -> io::Result<()> {
        let prev_bg = self.bg.replace(bg);

        if self.bg != prev_bg {
            self.bytes.write(&self.bg_colors[bg as usize])?;
        }
        Ok(())
    }

    #[inline]
    pub fn reset_color(&mut self) -> io::Result<usize> {
        self.bytes.write(b"\x1b[m")
    }

    #[inline]
    pub fn clear(&mut self) {
        self.bytes.clear();
        self.fg = None;
        self.bg = None;
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..]
    }
}
