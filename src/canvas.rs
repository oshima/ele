use std::env;
use std::io::{self, Write};

use crate::face::{Bg, Fg};

#[derive(Clone, Copy)]
pub enum Term {
    TrueColor,
    Color256,
    Color16,
}

impl Term {
    fn detect() -> Self {
        match env::var("COLORTERM") {
            Ok(val) if val == "truecolor" => Self::TrueColor,
            _ => match env::var("TERM") {
                Ok(val) if val.contains("256color") => Self::Color256,
                _ => Self::Color16,
            },
        }
    }
}

pub struct Canvas {
    pub term: Term,
    bytes: Vec<u8>,
    fg: Option<Fg>,
    bg: Option<Bg>,
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
            term: Term::detect(),
            bytes: Vec::new(),
            fg: None,
            bg: None,
            fg_colors: Default::default(),
            bg_colors: Default::default(),
        };
        canvas.map_colors();
        canvas
    }

    fn map_colors(&mut self) {
        // TODO: load config file

        // default: Tomorrow Night Bright
        match self.term {
            Term::TrueColor => {
                self.map_fg_color(Fg::Default, fg_color!(234, 234, 234));
                self.map_fg_color(Fg::Keyword, fg_color!(195, 151, 216));
                self.map_fg_color(Fg::Type, fg_color!(231, 197, 71));
                self.map_fg_color(Fg::Module, fg_color!(112, 192, 177));
                self.map_fg_color(Fg::Variable, fg_color!(231, 140, 69));
                self.map_fg_color(Fg::Function, fg_color!(122, 166, 218));
                self.map_fg_color(Fg::Macro, fg_color!(112, 192, 177));
                self.map_fg_color(Fg::String, fg_color!(185, 202, 74));
                self.map_fg_color(Fg::Comment, fg_color!(150, 152, 150));
                self.map_fg_color(Fg::Prompt, fg_color!(122, 166, 218));
                self.map_fg_color(Fg::Match, fg_color!(0, 0, 0));
                self.map_fg_color(Fg::CurrentMatch, fg_color!(0, 0, 0));

                self.map_bg_color(Bg::Default, bg_color!(0, 0, 0));
                self.map_bg_color(Bg::Region, bg_color!(66, 66, 66));
                self.map_bg_color(Bg::StatusBar, bg_color!(28, 28, 28));
                self.map_bg_color(Bg::Match, bg_color!(231, 197, 71));
                self.map_bg_color(Bg::CurrentMatch, bg_color!(231, 140, 69));
            }
            Term::Color256 => {
                self.map_fg_color(Fg::Default, fg_color256!(255));
                self.map_fg_color(Fg::Keyword, fg_color256!(182));
                self.map_fg_color(Fg::Type, fg_color256!(179));
                self.map_fg_color(Fg::Module, fg_color256!(115));
                self.map_fg_color(Fg::Variable, fg_color256!(173));
                self.map_fg_color(Fg::Function, fg_color256!(110));
                self.map_fg_color(Fg::Macro, fg_color256!(115));
                self.map_fg_color(Fg::String, fg_color256!(143));
                self.map_fg_color(Fg::Comment, fg_color256!(246));
                self.map_fg_color(Fg::Prompt, fg_color256!(110));
                self.map_fg_color(Fg::Match, fg_color256!(16));
                self.map_fg_color(Fg::CurrentMatch, fg_color256!(16));

                self.map_bg_color(Bg::Default, bg_color256!(16));
                self.map_bg_color(Bg::Region, bg_color256!(238));
                self.map_bg_color(Bg::StatusBar, bg_color256!(234));
                self.map_bg_color(Bg::Match, bg_color256!(179));
                self.map_bg_color(Bg::CurrentMatch, bg_color256!(173));
            }
            Term::Color16 => {
                self.map_fg_color(Fg::Default, fg_color16!(white));
                self.map_fg_color(Fg::Keyword, fg_color16!(magenta));
                self.map_fg_color(Fg::Type, fg_color16!(yellow));
                self.map_fg_color(Fg::Module, fg_color16!(cyan));
                self.map_fg_color(Fg::Variable, fg_color16!(red));
                self.map_fg_color(Fg::Function, fg_color16!(blue));
                self.map_fg_color(Fg::Macro, fg_color16!(cyan));
                self.map_fg_color(Fg::String, fg_color16!(green));
                self.map_fg_color(Fg::Comment, fg_color16!(cyan));
                self.map_fg_color(Fg::Prompt, fg_color16!(blue));
                self.map_fg_color(Fg::Match, fg_color16!(black));
                self.map_fg_color(Fg::CurrentMatch, fg_color16!(black));

                self.map_bg_color(Bg::Default, bg_color16!(black));
                self.map_bg_color(Bg::Region, bg_color16!(bright_black));
                self.map_bg_color(Bg::StatusBar, bg_color16!(bright_black));
                self.map_bg_color(Bg::Match, bg_color16!(yellow));
                self.map_bg_color(Bg::CurrentMatch, bg_color16!(red));
            }
        }
    }

    fn map_fg_color(&mut self, fg: Fg, color: &[u8]) {
        self.fg_colors[fg as usize].extend_from_slice(color);
    }

    fn map_bg_color(&mut self, bg: Bg, color: &[u8]) {
        self.bg_colors[bg as usize].extend_from_slice(color);
    }

    #[inline]
    pub fn set_fg_color(&mut self, fg: Fg) -> io::Result<()> {
        if self.fg != Some(fg) {
            self.bytes.write(&self.fg_colors[fg as usize])?;
            self.fg.replace(fg);
        }
        Ok(())
    }

    #[inline]
    pub fn set_bg_color(&mut self, bg: Bg) -> io::Result<()> {
        if self.bg != Some(bg) {
            self.bytes.write(&self.bg_colors[bg as usize])?;
            self.bg.replace(bg);
        }
        Ok(())
    }

    #[inline]
    pub fn reset_color(&mut self) -> io::Result<()> {
        self.bytes.write(b"\x1b[m")?;
        self.fg = None;
        self.bg = None;
        Ok(())
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
