extern crate unicode_width;

use std::cmp;
use std::io;
use std::ops::Range;
use unicode_width::UnicodeWidthChar;

use crate::canvas::Canvas;
use crate::face::Face;
use crate::uint_vec::UintVec;

const TAB_WIDTH: usize = 4;
const TOMBSTONE: usize = 0;

pub type HlContext = u32;

pub struct Row {
    pub string: String,
    x_to_idx: Option<Box<UintVec>>,
    pub hl_context: HlContext,
    pub faces: Vec<Face>,
}

impl Row {
    pub fn new(string: String) -> Self {
        let mut row = Self {
            string,
            x_to_idx: None,
            hl_context: 0,
            faces: Vec::new(),
        };
        row.update();
        row
    }

    fn char_at(&self, x: usize) -> char {
        let idx = self.byte_index(x);
        self.string[idx..].chars().next().unwrap()
    }

    #[inline]
    fn char_width(&self, x: usize) -> usize {
        let mut width = 1;
        while !self.is_char_boundary(x + width) {
            width += 1;
        }
        width
    }

    #[inline]
    fn byte_index(&self, x: usize) -> usize {
        match self.x_to_idx.as_ref() {
            Some(v) => v.get(x),
            None => x,
        }
    }

    #[inline]
    fn is_char_boundary(&self, x: usize) -> bool {
        match self.x_to_idx.as_ref() {
            Some(v) => x == 0 || v.get(x) != TOMBSTONE,
            None => true,
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

    pub fn insert(&mut self, x: usize, ch: char) {
        let idx = self.byte_index(x);
        self.string.insert(idx, ch);
        if self.x_to_idx.is_some() || ch == '\t' || !ch.is_ascii() {
            self.update();
        }
    }

    pub fn remove(&mut self, x: usize) {
        let idx = self.byte_index(x);
        self.string.remove(idx);
        if self.x_to_idx.is_some() {
            self.update();
        }
    }

    pub fn clear(&mut self) {
        self.string.clear();
        if self.x_to_idx.is_some() {
            self.update();
        }
    }

    pub fn truncate(&mut self, x: usize) {
        let idx = self.byte_index(x);
        self.string.truncate(idx);
        if self.x_to_idx.is_some() {
            self.update();
        }
    }

    pub fn split_off(&mut self, x: usize) -> String {
        let idx = self.byte_index(x);
        let string = self.string.split_off(idx);
        if self.x_to_idx.is_some() {
            self.update();
        }
        string
    }

    pub fn push_str(&mut self, string: &str) {
        self.string.push_str(string);
        self.update();
    }

    pub fn remove_str(&mut self, from_x: usize, to_x: usize) {
        let from = self.byte_index(from_x);
        let to = self.byte_index(to_x);
        let string = self.string.split_off(to);
        self.string.truncate(from);
        self.string.push_str(&string);
        if self.x_to_idx.is_some() {
            self.update();
        }
    }

    fn update(&mut self) {
        if self.string.is_empty() {
            self.x_to_idx = None;
            return;
        }

        let x_to_idx = self.x_to_idx.get_or_insert(Box::new(UintVec::new()));
        let mut need_mappings = false;

        x_to_idx.clear();

        for (idx, ch) in self.string.char_indices() {
            let width = match ch {
                '\t' => TAB_WIDTH - x_to_idx.len() % TAB_WIDTH,
                _ => ch.width().unwrap_or(0),
            };
            for i in 0..width {
                x_to_idx.push(if i == 0 { idx } else { TOMBSTONE });
            }
            if ch == '\t' || !ch.is_ascii() {
                need_mappings = true;
            }
        }

        if need_mappings {
            x_to_idx.push(self.string.len());
        } else {
            self.x_to_idx = None;
        }
    }

    pub fn draw(&self, canvas: &mut Canvas, x_range: Range<usize>) -> io::Result<()> {
        if self.max_x() <= x_range.start {
            return Ok(());
        }

        let start_x = self.next_fit_x(x_range.start);
        let end_x = self.prev_fit_x(x_range.end);
        let start = self.byte_index(start_x);
        let end = self.byte_index(end_x);

        for _ in 0..(start_x - x_range.start) {
            canvas.write(b" ")?;
        }

        let mut x = start_x;
        let mut prev_face = Face::Background;

        for (idx, ch) in self.string[start..end].char_indices() {
            let idx = start + idx;
            let width = self.char_width(x);
            let face = self.faces[idx];

            if face != prev_face {
                canvas.set_color(face)?;
                prev_face = face;
            }

            if ch == '\t' {
                for _ in 0..width {
                    canvas.write(b" ")?;
                }
            } else {
                let s = &self.string[idx..(idx + ch.len_utf8())];
                canvas.write(s.as_bytes())?;
            }

            x += width;
        }

        Ok(())
    }
}
