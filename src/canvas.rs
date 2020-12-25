use std::env;
use std::io::{self, Write};

use crate::face::Face;

#[derive(Clone, Copy)]
pub enum Term {
    TrueColor,
    Color256,
    Color16,
}

pub struct Canvas {
    pub term: Term,
    bytes: Vec<u8>,
    colors: [Vec<u8>; 14],
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
            colors: Default::default(),
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
                self.define_color(Face::Default, b"\x1b[38;2;234;234;234m");
                self.define_color(Face::Keyword, b"\x1b[38;2;195;151;216m");
                self.define_color(Face::Type, b"\x1b[38;2;231;197;71m");
                self.define_color(Face::Module, b"\x1b[38;2;112;192;177m");
                self.define_color(Face::Variable, b"\x1b[38;2;231;140;69m");
                self.define_color(Face::Function, b"\x1b[38;2;122;166;218m");
                self.define_color(Face::Macro, b"\x1b[38;2;112;192;177m");
                self.define_color(Face::String, b"\x1b[38;2;185;202;74m");
                self.define_color(Face::Comment, b"\x1b[38;2;150;152;150m");
                self.define_color(Face::Prompt, b"\x1b[38;2;122;166;218m");
                self.define_color(Face::Background, b"\x1b[48;2;0;0;0m");
                self.define_color(Face::Match, b"\x1b[48;2;231;197;71m\x1b[38;2;0;0;0m");
                self.define_color(Face::CurrentMatch, b"\x1b[48;2;231;140;69m\x1b[38;2;0;0;0m");
                self.define_color(Face::StatusBar, b"\x1b[48;2;28;28;28m");
            }
            Term::Color256 => {
                self.define_color(Face::Default, b"\x1b[38;5;255m");
                self.define_color(Face::Keyword, b"\x1b[38;5;182m");
                self.define_color(Face::Type, b"\x1b[38;5;179m");
                self.define_color(Face::Module, b"\x1b[38;5;115m");
                self.define_color(Face::Variable, b"\x1b[38;5;173m");
                self.define_color(Face::Function, b"\x1b[38;5;110m");
                self.define_color(Face::Macro, b"\x1b[38;5;115m");
                self.define_color(Face::String, b"\x1b[38;5;143m");
                self.define_color(Face::Comment, b"\x1b[38;5;246m");
                self.define_color(Face::Prompt, b"\x1b[38;5;110m");
                self.define_color(Face::Background, b"\x1b[48;5;16m");
                self.define_color(Face::Match, b"\x1b[48;5;179m\x1b[38;5;16m");
                self.define_color(Face::CurrentMatch, b"\x1b[48;5;173m\x1b[38;5;16m");
                self.define_color(Face::StatusBar, b"\x1b[48;5;234m");
            }
            Term::Color16 => {
                self.define_color(Face::Default, b"\x1b[39m");
                self.define_color(Face::Keyword, b"\x1b[35m");
                self.define_color(Face::Type, b"\x1b[33m");
                self.define_color(Face::Module, b"\x1b[36m");
                self.define_color(Face::Variable, b"\x1b[31m");
                self.define_color(Face::Function, b"\x1b[34m");
                self.define_color(Face::Macro, b"\x1b[36m");
                self.define_color(Face::String, b"\x1b[32m");
                self.define_color(Face::Comment, b"\x1b[36m");
                self.define_color(Face::Prompt, b"\x1b[34m");
                self.define_color(Face::Background, b"\x1b[40m");
                self.define_color(Face::Match, b"\x1b[43m\x1b[30m");
                self.define_color(Face::CurrentMatch, b"\x1b[41m\x1b[30m");
                self.define_color(Face::StatusBar, b"\x1b[40m");
            }
        }
    }

    fn define_color(&mut self, face: Face, color: &[u8]) {
        self.colors[face as usize].extend_from_slice(color);
    }

    #[inline]
    pub fn set_color(&mut self, face: Face) -> io::Result<usize> {
        self.bytes.write(&self.colors[face as usize])
    }

    #[inline]
    pub fn reset_color(&mut self) -> io::Result<usize> {
        self.bytes.write(b"\x1b[m")
    }

    #[inline]
    pub fn clear(&mut self) {
        self.bytes.clear()
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..]
    }
}
