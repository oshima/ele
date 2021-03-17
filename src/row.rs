extern crate unicode_width;

use std::cmp;
use std::io::{self, Write};
use std::ops::Range;
use unicode_width::UnicodeWidthChar;

use crate::canvas::Canvas;
use crate::face::{Bg, Fg};
use crate::util::UintVec;

pub const TAB_WIDTH: usize = 4;
const ZWJ_WIDTH: usize = 1;
const TOMBSTONE: usize = 0;

#[inline]
fn char_width(x: usize, ch: char) -> usize {
    match ch {
        '\t' => TAB_WIDTH - x % TAB_WIDTH,
        '\u{200d}' => ZWJ_WIDTH,
        _ => ch.width().unwrap_or(0),
    }
}

pub type HlContext = u32;

pub struct Row {
    pub string: String,
    x_to_idx: Option<Box<UintVec>>,
    pub hl_context: HlContext,
    pub faces: Vec<(Fg, Bg)>,
    pub trailing_bg: Bg,
}

impl Row {
    pub fn new(string: String) -> Self {
        let mut row = Self {
            string,
            x_to_idx: None,
            hl_context: 0,
            faces: Vec::new(),
            trailing_bg: Bg::Default,
        };
        row.update_mappings();
        row
    }

    #[inline]
    pub fn x_to_idx(&self, x: usize) -> usize {
        match self.x_to_idx.as_ref() {
            Some(v) => v.get(x),
            None => x,
        }
    }

    pub fn idx_to_x(&self, idx: usize) -> usize {
        match self.x_to_idx.as_ref() {
            Some(v) => {
                let mut x = 0;
                loop {
                    if v.get(x) == idx {
                        break x;
                    }
                    x += 1;
                }
            }
            None => idx,
        }
    }

    #[inline]
    pub fn max_x(&self) -> usize {
        match self.x_to_idx.as_ref() {
            Some(v) => v.len() - 1,
            None => self.string.len(),
        }
    }

    pub fn prev_x(&self, x: usize) -> usize {
        let mut x = x - 1;
        while !self.is_char_boundary(x) {
            x -= 1;
        }
        x
    }

    pub fn next_x(&self, x: usize) -> usize {
        let mut x = x + 1;
        while !self.is_char_boundary(x) {
            x += 1;
        }
        x
    }

    pub fn prev_fit_x(&self, proposed_x: usize) -> usize {
        let mut x = cmp::min(proposed_x, self.max_x());
        while !self.is_char_boundary(x) {
            x -= 1;
        }
        x
    }

    pub fn next_fit_x(&self, proposed_x: usize) -> usize {
        let mut x = cmp::min(proposed_x, self.max_x());
        while !self.is_char_boundary(x) {
            x += 1;
        }
        x
    }

    pub fn first_letter_x(&self) -> usize {
        let mut x = 0;
        let max_x = self.max_x();
        while x < max_x {
            if !self.char_at(x).is_ascii_whitespace() {
                return x;
            }
            x = self.next_x(x);
        }
        x
    }

    fn char_at(&self, x: usize) -> char {
        let idx = self.x_to_idx(x);
        self.string[idx..].chars().next().unwrap()
    }

    #[inline]
    fn is_char_boundary(&self, x: usize) -> bool {
        match self.x_to_idx.as_ref() {
            Some(v) => x == 0 || v.get(x) != TOMBSTONE,
            None => true,
        }
    }

    pub fn read(&self, x1: usize, x2: usize) -> String {
        self.string[self.x_to_idx(x1)..self.x_to_idx(x2)].to_string()
    }

    pub fn insert(&mut self, x: usize, ch: char) {
        let idx = self.x_to_idx(x);
        self.string.insert(idx, ch);
        if self.x_to_idx.is_some() || ch == '\t' || !ch.is_ascii() {
            self.update_mappings();
        }
    }

    pub fn remove(&mut self, x: usize) -> String {
        let from = self.x_to_idx(x);
        let to = self.x_to_idx(self.next_x(x));
        let string = self.string.split_off(to);
        let removed = self.string.split_off(from);
        self.string.push_str(&string);
        if self.x_to_idx.is_some() {
            self.update_mappings();
        }
        removed
    }

    pub fn clear(&mut self) {
        self.string.clear();
        if self.x_to_idx.is_some() {
            self.update_mappings();
        }
    }

    pub fn truncate(&mut self, x: usize) {
        let idx = self.x_to_idx(x);
        self.string.truncate(idx);
        if self.x_to_idx.is_some() {
            self.update_mappings();
        }
    }

    pub fn split_off(&mut self, x: usize) -> String {
        let idx = self.x_to_idx(x);
        let string = self.string.split_off(idx);
        if self.x_to_idx.is_some() {
            self.update_mappings();
        }
        string
    }

    pub fn push_str(&mut self, string: &str) {
        self.string.push_str(string);
        self.update_mappings();
    }

    pub fn insert_str(&mut self, x: usize, string: &str) {
        let idx = self.x_to_idx(x);
        self.string.insert_str(idx, string);
        self.update_mappings();
    }

    pub fn remove_str(&mut self, from_x: usize, to_x: usize) -> String {
        let from = self.x_to_idx(from_x);
        let to = self.x_to_idx(to_x);
        let string = self.string.split_off(to);
        let removed = self.string.split_off(from);
        self.string.push_str(&string);
        if self.x_to_idx.is_some() {
            self.update_mappings();
        }
        removed
    }

    fn update_mappings(&mut self) {
        let x_to_idx = self.x_to_idx.get_or_insert(Box::new(UintVec::new()));
        let mut need_mappings = false;

        x_to_idx.clear();

        for (idx, ch) in self.string.char_indices() {
            let width = char_width(x_to_idx.len(), ch);

            for i in 0..width {
                x_to_idx.push(if i == 0 { idx } else { TOMBSTONE });
            }
            if ch == '\t' || !ch.is_ascii() {
                need_mappings = true;
            }
        }

        if !need_mappings {
            self.x_to_idx = None;
            return;
        }

        x_to_idx.push(self.string.len());
    }

    pub fn draw(&self, canvas: &mut Canvas, x_range: Range<usize>) -> io::Result<()> {
        let start_x = self.next_fit_x(x_range.start);
        let end_x = self.prev_fit_x(x_range.end);
        let start = self.x_to_idx(start_x);
        let end = self.x_to_idx(end_x);

        if x_range.start < start_x {
            canvas.set_bg_color(self.faces[start - 1].1)?;
            for _ in 0..(start_x - x_range.start) {
                canvas.write(b" ")?;
            }
        }

        let mut x = start_x;

        for (idx, ch) in self.string[start..end].char_indices() {
            let idx = start + idx;
            let width = char_width(x, ch);
            let (fg, bg) = self.faces[idx];

            canvas.set_fg_color(fg)?;
            canvas.set_bg_color(bg)?;

            match ch {
                '\t' => {
                    for _ in 0..width {
                        canvas.write(b" ")?;
                    }
                }
                '\u{200d}' => {
                    canvas.write(b"\x1b[4m")?;
                    for _ in 0..width {
                        canvas.write(b" ")?;
                    }
                    canvas.write(b"\x1b[24m")?;
                }
                _ => {
                    let s = &self.string[idx..(idx + ch.len_utf8())];
                    canvas.write(s.as_bytes())?;
                }
            };

            x += width;
        }

        if end_x < x_range.end && x_range.end <= self.max_x() {
            canvas.set_bg_color(self.faces[end].1)?;
            for _ in 0..(x_range.end - end_x) {
                canvas.write(b" ")?;
            }
        }

        canvas.set_bg_color(self.trailing_bg)?;
        Ok(())
    }
}
