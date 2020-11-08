extern crate unicode_width;

use std::io::{self, Write};
use unicode_width::UnicodeWidthChar;

use crate::syntax::{Hl, HlContext};
use crate::uint_vec::UintVec;

const TAB_WIDTH: usize = 4;
const TOMBSTONE: usize = 0;

// std::mem::size_of::<Row>() => 104
pub struct Row {
    pub string: String,
    pub x_to_idx: UintVec,
    pub hl_context: HlContext,
    pub hls: Vec<Hl>,
}

impl Row {
    pub fn new(string: String) -> Self {
        let mut row = Self {
            string,
            x_to_idx: UintVec::new(),
            hl_context: 0,
            hls: Vec::new(),
        };
        row.update();
        row
    }

    #[inline]
    pub fn max_x(&self) -> usize {
        self.x_to_idx.len() - 1
    }

    pub fn prev_x(&self, x: usize) -> usize {
        let mut x = x - 1;
        while self.is_invalid_x(x) {
            x -= 1;
        }
        x
    }

    pub fn next_x(&self, x: usize) -> usize {
        let mut x = x + 1;
        while self.is_invalid_x(x) {
            x += 1;
        }
        x
    }

    pub fn suitable_prev_x(&self, proposed_x: usize) -> usize {
        if self.max_x() <= proposed_x {
            return self.max_x();
        }
        let mut x = proposed_x;
        while self.is_invalid_x(x) {
            x -= 1;
        }
        x
    }

    pub fn suitable_next_x(&self, proposed_x: usize) -> usize {
        if self.max_x() <= proposed_x {
            return self.max_x();
        }
        let mut x = proposed_x;
        while self.is_invalid_x(x) {
            x += 1;
        }
        x
    }

    pub fn first_letter_x(&self) -> usize {
        for x in 0..self.max_x() {
            if !self.char_at(x).is_ascii_whitespace() {
                return x;
            }
        }
        self.max_x()
    }

    fn char_at(&self, x: usize) -> char {
        let idx = self.x_to_idx.get(x);
        self.string[idx..].chars().next().unwrap()
    }

    fn char_width(&self, x: usize) -> usize {
        let mut width = 1;
        while self.is_invalid_x(x + width) {
            width += 1;
        }
        width
    }

    #[inline]
    fn is_invalid_x(&self, x: usize) -> bool {
        x > 0 && self.x_to_idx.get(x) == TOMBSTONE
    }

    pub fn insert(&mut self, x: usize, ch: char) {
        let idx = self.x_to_idx.get(x);
        self.string.insert(idx, ch);
        self.update();
    }

    pub fn remove(&mut self, x: usize) {
        let idx = self.x_to_idx.get(x);
        self.string.remove(idx);
        self.update();
    }

    pub fn clear(&mut self) {
        self.string.clear();
        self.update();
    }

    pub fn truncate(&mut self, x: usize) {
        let idx = self.x_to_idx.get(x);
        self.string.truncate(idx);
        self.update();
    }

    pub fn split_off(&mut self, x: usize) -> String {
        let idx = self.x_to_idx.get(x);
        let string = self.string.split_off(idx);
        self.update();
        string
    }

    pub fn push_str(&mut self, string: &str) {
        self.string.push_str(string);
        self.update();
    }

    pub fn remove_str(&mut self, from_x: usize, to_x: usize) {
        let from = self.x_to_idx.get(from_x);
        let to = self.x_to_idx.get(to_x);
        let string = self.string.split_off(to);
        self.string.truncate(from);
        self.string.push_str(&string);
        self.update();
    }

    fn update(&mut self) {
        self.x_to_idx.clear();

        for (idx, ch) in self.string.char_indices() {
            if ch == '\t' {
                for i in 0..(TAB_WIDTH - self.x_to_idx.len() % TAB_WIDTH) {
                    self.x_to_idx.push(if i == 0 { idx } else { TOMBSTONE });
                }
            } else {
                for i in 0..ch.width().unwrap_or(0) {
                    self.x_to_idx.push(if i == 0 { idx } else { TOMBSTONE });
                }
            }
        }
        self.x_to_idx.push(self.string.len());
    }

    pub fn draw(&self, canvas: &mut Vec<u8>, coloff: usize, width: usize) -> io::Result<()> {
        if self.max_x() <= coloff {
            return Ok(());
        }

        let start_x = self.suitable_next_x(coloff);
        let end_x = self.suitable_prev_x(coloff + width);
        let start = self.x_to_idx.get(start_x);
        let end = self.x_to_idx.get(end_x);

        let mut x = start_x;
        let mut hl = Hl::Default;

        for _ in 0..(start_x - coloff) {
            canvas.write(b" ")?;
        }

        for (idx, ch) in self.string[start..end].char_indices() {
            let idx = start + idx;
            let ch_width = self.char_width(x);

            if self.hls[idx] != hl {
                match self.hls[idx] {
                    Hl::Default => canvas.write(b"\x1b[m")?,
                    Hl::Keyword => canvas.write(b"\x1b[35m")?,
                    Hl::Type => canvas.write(b"\x1b[33m")?,
                    Hl::Module => canvas.write(b"\x1b[36m")?,
                    Hl::Variable => canvas.write(b"\x1b[31m")?,
                    Hl::Function => canvas.write(b"\x1b[34m")?,
                    Hl::Macro => canvas.write(b"\x1b[36m")?,
                    Hl::String => canvas.write(b"\x1b[32m")?,
                    Hl::Comment => canvas.write(b"\x1b[36m")?,
                };
                hl = self.hls[idx];
            }

            if ch == '\t' {
                for _ in 0..ch_width {
                    canvas.write(b" ")?;
                }
            } else {
                let s = &self.string[idx..(idx + ch.len_utf8())];
                canvas.write(s.as_bytes())?;
            }

            x += ch_width;
        }

        canvas.write(b"\x1b[m")?;
        Ok(())
    }
}
