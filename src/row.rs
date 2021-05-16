extern crate unicode_width;

use std::cmp;
use std::io::{self, Write};
use std::ops::Range;
use unicode_width::UnicodeWidthChar;

use crate::canvas::Canvas;
use crate::face::{Bg, Fg};
use crate::util::UintVec;

const TAB_WIDTH: usize = 4;
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

fn str_width(x: usize, string: &str) -> usize {
    string.chars().fold(0, |w, ch| w + char_width(x + w, ch))
}

pub struct Row {
    pub string: String,
    x_to_idx: Option<Box<UintVec>>,
    pub hl_context: Option<String>,
    pub faces: Vec<(Fg, Bg)>,
    pub trailing_bg: Bg,
    pub indent_level: usize,
}

impl Row {
    pub fn new(string: String) -> Self {
        let mut row = Self {
            string,
            x_to_idx: None,
            hl_context: None,
            faces: Vec::new(),
            trailing_bg: Bg::Default,
            indent_level: 0,
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
    pub fn last_x(&self) -> usize {
        match self.x_to_idx.as_ref() {
            Some(v) => v.len() - 1,
            None => self.string.len(),
        }
    }

    pub fn prev_x(&self, x: usize) -> Option<usize> {
        if x > 0 {
            let mut x = x - 1;
            while !self.is_char_boundary(x) {
                x -= 1;
            }
            Some(x)
        } else {
            None
        }
    }

    pub fn next_x(&self, x: usize) -> Option<usize> {
        if x < self.last_x() {
            let mut x = x + 1;
            while !self.is_char_boundary(x) {
                x += 1;
            }
            Some(x)
        } else {
            None
        }
    }

    pub fn prev_fit_x(&self, proposed_x: usize) -> usize {
        let mut x = cmp::min(proposed_x, self.last_x());
        while !self.is_char_boundary(x) {
            x -= 1;
        }
        x
    }

    pub fn next_fit_x(&self, proposed_x: usize) -> usize {
        let mut x = cmp::min(proposed_x, self.last_x());
        while !self.is_char_boundary(x) {
            x += 1;
        }
        x
    }

    pub fn first_word_x(&self) -> Option<usize> {
        if self.is_word_boundary(0) {
            Some(0)
        } else {
            self.next_word_x(0)
        }
    }

    pub fn last_word_x(&self) -> Option<usize> {
        self.prev_word_x(self.last_x())
    }

    pub fn prev_word_x(&self, x: usize) -> Option<usize> {
        let mut x = self.prev_x(x)?;
        while !self.is_word_boundary(x) {
            x = self.prev_x(x)?;
        }
        Some(x)
    }

    pub fn next_word_x(&self, x: usize) -> Option<usize> {
        let mut x = self.next_x(x)?;
        while !self.is_word_boundary(x) {
            x = self.next_x(x)?;
        }
        Some(x)
    }

    pub fn beginning_of_code_x(&self) -> usize {
        let mut x = 0;
        while let Some(next_x) = self.next_x(x) {
            if !self.char_at(x).is_ascii_whitespace() {
                return x;
            }
            x = next_x;
        }
        x
    }

    #[inline]
    fn is_char_boundary(&self, x: usize) -> bool {
        match self.x_to_idx.as_ref() {
            Some(v) => x == 0 || v.get(x) != TOMBSTONE,
            None => true,
        }
    }

    fn is_word_boundary(&self, x: usize) -> bool {
        let ch = self.char_at(x);

        if ch.is_ascii_whitespace() || ch.is_ascii_punctuation() {
            false
        } else if let Some(prev_x) = self.prev_x(x) {
            let prev_ch = self.char_at(prev_x);
            prev_ch.is_ascii_whitespace() || prev_ch.is_ascii_punctuation()
        } else {
            true
        }
    }

    fn char_at(&self, x: usize) -> char {
        let idx = self.x_to_idx(x);
        self.string[idx..].chars().next().unwrap_or('\n')
    }

    pub fn clear(&mut self) {
        self.string.clear();
        if self.x_to_idx.is_some() {
            self.update_mappings();
        }
    }

    pub fn read(&self, x1: usize, x2: usize) -> String {
        self.string[self.x_to_idx(x1)..self.x_to_idx(x2)].to_string()
    }

    pub fn indent_part(&self) -> &str {
        let len = self
            .string
            .char_indices()
            .find(|&(_, ch)| !ch.is_ascii_whitespace())
            .map_or(self.string.len(), |(idx, _)| idx);

        &self.string[..len]
    }

    pub fn indent(&mut self, string: &str) -> (String, isize) {
        let len = self.indent_part().len();
        let code_part = self.string.split_off(len);
        let indent_part = self.string.split_off(0);
        let diff = str_width(0, string) as isize - str_width(0, &indent_part) as isize;
        self.string.push_str(string);
        self.string.push_str(&code_part);
        (indent_part, diff)
    }

    pub fn push_str(&mut self, string: &str) {
        self.string.push_str(string);
        self.update_mappings();
    }

    pub fn insert_str(&mut self, x: usize, string: &str) -> usize {
        let idx = self.x_to_idx(x);
        self.string.insert_str(idx, string);
        self.update_mappings();
        x + str_width(x, string)
    }

    pub fn remove_str(&mut self, x1: usize, x2: usize) -> String {
        let idx1 = self.x_to_idx(x1);
        let idx2 = self.x_to_idx(x2);
        let string = self.string.split_off(idx2);
        let removed = self.string.split_off(idx1);
        self.string.push_str(&string);
        if self.x_to_idx.is_some() {
            self.update_mappings();
        }
        removed
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
            canvas.write_repeat(b" ", start_x - x_range.start)?;
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
                    canvas.write_repeat(b" ", width)?;
                }
                '\u{200d}' => {
                    canvas.write(b"\x1b[4m")?;
                    canvas.write_repeat(b" ", width)?;
                    canvas.write(b"\x1b[24m")?;
                }
                _ => {
                    let s = &self.string[idx..(idx + ch.len_utf8())];
                    canvas.write(s.as_bytes())?;
                }
            };

            x += width;
        }

        if end_x < x_range.end && x_range.end <= self.last_x() {
            canvas.set_bg_color(self.faces[end].1)?;
            canvas.write_repeat(b" ", x_range.end - end_x)?;
        }

        canvas.set_bg_color(self.trailing_bg)
    }
}
