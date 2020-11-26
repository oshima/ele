use std::env;
use std::io::{self, Write};

use crate::hl::Hl;

#[derive(Clone, Copy)]
pub enum Term {
    TrueColor,
    Color256,
    Color16,
}

pub struct Canvas {
    pub term: Term,
    bytes: Vec<u8>,
    colors: [Vec<u8>; 11],
}

impl Canvas {
    pub fn new() -> Self {
        let term = Self::detect_term();

        Self {
            term,
            bytes: Vec::new(),
            colors: Self::load_colors(term),
        }
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

    fn load_colors(term: Term) -> [Vec<u8>; 11] {
        // TODO: load config file

        // default: Tomorrow Night Bright
        match term {
            Term::TrueColor => [
                b"\x1b[38;2;234;234;234m".to_vec(), // Hl::Default
                b"\x1b[38;2;195;151;216m".to_vec(), // Hl::Keyword
                b"\x1b[38;2;231;197;71m".to_vec(),  // Hl::Type
                b"\x1b[38;2;112;192;177m".to_vec(), // Hl::Module
                b"\x1b[38;2;231;140;69m".to_vec(),  // Hl::Variable
                b"\x1b[38;2;122;166;218m".to_vec(), // Hl::Function
                b"\x1b[38;2;112;192;177m".to_vec(), // Hl::Macro
                b"\x1b[38;2;185;202;74m".to_vec(),  // Hl::String
                b"\x1b[38;2;150;152;150m".to_vec(), // Hl::Comment
                b"\x1b[48;2;0;0;0m".to_vec(),       // Hl::Background
                b"\x1b[48;2;28;28;28m".to_vec(),    // Hl::StatusBar
            ],
            Term::Color256 => [
                b"\x1b[38;5;255m".to_vec(), // Hl::Default
                b"\x1b[38;5;182m".to_vec(), // Hl::Keyword
                b"\x1b[38;5;179m".to_vec(), // Hl::Type
                b"\x1b[38;5;115m".to_vec(), // Hl::Module
                b"\x1b[38;5;173m".to_vec(), // Hl::Variable
                b"\x1b[38;5;110m".to_vec(), // Hl::Function
                b"\x1b[38;5;115m".to_vec(), // Hl::Macro
                b"\x1b[38;5;143m".to_vec(), // Hl::String
                b"\x1b[38;5;246m".to_vec(), // Hl::Comment
                b"\x1b[48;5;16m".to_vec(),  // Hl::Background
                b"\x1b[48;5;234m".to_vec(), // Hl::StatusBar
            ],
            Term::Color16 => [
                b"\x1b[39m".to_vec(), // Hl::Default
                b"\x1b[35m".to_vec(), // Hl::Keyword
                b"\x1b[33m".to_vec(), // Hl::Type
                b"\x1b[36m".to_vec(), // Hl::Module
                b"\x1b[31m".to_vec(), // Hl::Variable
                b"\x1b[34m".to_vec(), // Hl::Function
                b"\x1b[36m".to_vec(), // Hl::Macro
                b"\x1b[32m".to_vec(), // Hl::String
                b"\x1b[36m".to_vec(), // Hl::Comment
                b"\x1b[40m".to_vec(), // Hl::Background
                b"\x1b[40m".to_vec(), // Hl::StatusBar
            ],
        }
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..]
    }

    #[inline]
    pub fn clear(&mut self) {
        self.bytes.clear()
    }

    #[inline]
    pub fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes.write(bytes)
    }

    #[inline]
    pub fn set_color(&mut self, hl: Hl) -> io::Result<usize> {
        self.bytes.write(&self.colors[hl as usize])
    }

    #[inline]
    pub fn reset_color(&mut self) -> io::Result<usize> {
        self.bytes.write(b"\x1b[m")
    }
}
