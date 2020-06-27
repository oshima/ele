extern crate unicode_width;

use std::cmp;
use std::io::{self, Write};
use unicode_width::UnicodeWidthChar;

const TAB_WIDTH: usize = 4;

pub struct Row {
    pub string: String,
    pub render: String,
    pub max_cx: usize,
    pub max_rx: usize,
    pub cx_to_rx: Vec<usize>,
    pub rx_to_cx: Vec<usize>,
    pub cx_to_idx: Vec<usize>,
    pub rx_to_idx: Vec<usize>,
}

impl Row {
    pub fn new(string: String) -> Self {
        let mut row = Self {
            string,
            render: String::new(),
            max_cx: 0,
            max_rx: 0,
            cx_to_rx: vec![],
            rx_to_cx: vec![],
            cx_to_idx: vec![],
            rx_to_idx: vec![],
        };
        row.update();
        row
    }

    pub fn insert(&mut self, cx: usize, ch: char) {
        let idx = self.cx_to_idx[cx];
        self.string.insert(idx, ch);
        self.update();
    }

    pub fn remove(&mut self, cx: usize) {
        let idx = self.cx_to_idx[cx];
        self.string.remove(idx);
        self.update();
    }

    pub fn clear(&mut self) {
        self.string.clear();
        self.update();
    }

    pub fn truncate(&mut self, cx: usize) {
        let idx = self.cx_to_idx[cx];
        self.string.truncate(idx);
        self.update();
    }

    pub fn split_off(&mut self, cx: usize) -> String {
        let idx = self.cx_to_idx[cx];
        let string = self.string.split_off(idx);
        self.update();
        string
    }

    pub fn push_str(&mut self, string: &str) {
        self.string.push_str(string);
        self.update();
    }

    pub fn remove_str(&mut self, from_cx: usize, to_cx: usize) {
        let from_idx = self.cx_to_idx[from_cx];
        let to_idx = self.cx_to_idx[to_cx];
        let string = self.string.split_off(to_idx);
        self.string.truncate(from_idx);
        self.string.push_str(&string);
        self.update();
    }

    fn update(&mut self) {
        self.render.clear();
        self.cx_to_rx.clear();
        self.rx_to_cx.clear();
        self.cx_to_idx.clear();
        self.rx_to_idx.clear();

        for (cx, (idx, ch)) in self.string.char_indices().enumerate() {
            self.cx_to_rx.push(self.rx_to_cx.len());
            self.cx_to_idx.push(idx);

            if ch == '\t' {
                for _ in 0..(TAB_WIDTH - self.rx_to_idx.len() % TAB_WIDTH) {
                    self.rx_to_cx.push(cx);
                    self.rx_to_idx.push(self.render.len());
                    self.render.push(' ');
                }
            } else {
                for _ in 0..UnicodeWidthChar::width(ch).unwrap_or(0) {
                    self.rx_to_cx.push(cx);
                    self.rx_to_idx.push(self.render.len());
                }
                self.render.push(ch);
            }
        }

        self.max_cx = self.cx_to_rx.len();
        self.max_rx = self.rx_to_cx.len();
        self.cx_to_rx.push(self.max_rx);
        self.rx_to_cx.push(self.max_cx);
        self.cx_to_idx.push(self.string.len());
        self.rx_to_idx.push(self.render.len());
    }

    pub fn draw(&self, start_rx: usize, end_rx: usize, canvas: &mut Vec<u8>) -> io::Result<()> {
        if start_rx >= self.max_rx {
            return Ok(());
        }

        let truncate_start =
            start_rx > 0 && self.rx_to_idx[start_rx] == self.rx_to_idx[start_rx - 1];
        let truncate_end =
            end_rx <= self.max_rx && self.rx_to_idx[end_rx - 1] == self.rx_to_idx[end_rx];

        let start_rx = if truncate_start {
            start_rx + 1
        } else {
            start_rx
        };
        let end_rx = if truncate_end {
            end_rx - 1
        } else {
            cmp::min(end_rx, self.max_rx)
        };

        let start_idx = self.rx_to_idx[start_rx];
        let end_idx = self.rx_to_idx[end_rx];

        if truncate_start {
            canvas.write(b"\x1b[34m~\x1b[39m")?;
        }
        canvas.write(&self.render[start_idx..end_idx].as_bytes())?;
        if truncate_end {
            canvas.write(b"\x1b[34m~\x1b[39m")?;
        }
        Ok(())
    }
}
