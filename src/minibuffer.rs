use std::cmp;
use std::io::{self, Write};

use crate::key::Key;
use crate::row::Row;

pub struct Minibuffer {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    cx: usize,
    cx_min: usize,
    rx: usize,
    coloff: usize,
    row: Row,
}

impl Minibuffer {
    pub fn new() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            cx: 0,
            cx_min: 0,
            rx: 0,
            coloff: 0,
            row: Row::new(vec![]),
        }
    }

    pub fn set_position(&mut self, x: usize, y: usize, width: usize, height: usize) {
        self.x = x;
        self.y = y;
        self.width = width;
        self.height = height;
    }

    pub fn set_message(&mut self, text: &str) {
        self.row.truncate_chars(0);
        self.row.append_chars(&mut text.as_bytes().to_vec());
        self.cx = 0;
        self.cx_min = 0;
        self.rx = 0;
        self.coloff = 0;
    }

    pub fn set_prompt(&mut self, text: &str) {
        self.row.truncate_chars(0);
        self.row.append_chars(&mut text.as_bytes().to_vec());
        self.cx = self.row.chars.len();
        self.cx_min = self.cx;
        self.rx = self.row.cx_to_rx[self.cx];
        self.coloff = 0;
    }

    pub fn get_input(&self) -> String {
        self.row.chars[self.cx_min..]
            .iter()
            .map(|ch| char::from(*ch))
            .collect()
    }

    pub fn draw(&mut self, bufout: &mut Vec<u8>) -> io::Result<()> {
        bufout.write(format!("\x1b[{};{}H", self.y + 1, self.x + 1).as_bytes())?;

        if self.row.render.len() > self.coloff {
            let len = cmp::min(self.row.render.len() - self.coloff, self.width);
            bufout.write(&self.row.render[self.coloff..(self.coloff + len)])?;
        }

        bufout.write(b"\x1b[K")?;
        Ok(())
    }

    pub fn draw_cursor(&mut self, bufout: &mut Vec<u8>) -> io::Result<()> {
        bufout.write(
            format!(
                "\x1b[{};{}H",
                self.y + 1,
                self.x + self.rx - self.coloff + 1,
            )
            .as_bytes(),
        )?;
        Ok(())
    }

    pub fn process_keypress(&mut self, key: Key) {
        match key {
            Key::ArrowLeft | Key::Ctrl(b'B') => {
                if self.cx > self.cx_min {
                    self.cx -= 1;
                    self.rx = self.row.cx_to_rx[self.cx];
                }
            }
            Key::ArrowRight | Key::Ctrl(b'F') => {
                if self.cx < self.row.chars.len() {
                    self.cx += 1;
                    self.rx = self.row.cx_to_rx[self.cx];
                }
            }
            Key::Home | Key::Ctrl(b'A') | Key::Alt(b'<') => {
                self.cx = self.cx_min;
                self.rx = self.row.cx_to_rx[self.cx];
            }
            Key::End | Key::Ctrl(b'E') | Key::Alt(b'>') => {
                self.cx = self.row.chars.len();
                self.rx = self.row.cx_to_rx[self.cx];
            }
            Key::Backspace | Key::Ctrl(b'H') => {
                if self.cx > self.cx_min {
                    self.row.remove_char(self.cx - 1);
                    self.cx -= 1;
                    self.rx = self.row.cx_to_rx[self.cx];
                }
            }
            Key::Delete | Key::Ctrl(b'D') => {
                if self.cx < self.row.chars.len() {
                    self.row.remove_char(self.cx);
                }
            }
            Key::Ctrl(b'I') => {
                self.row.insert_char(self.cx, b'\t');
                self.cx += 1;
                self.rx = self.row.cx_to_rx[self.cx];
            }
            Key::Ctrl(b'K') => {
                self.row.truncate_chars(self.cx);
            }
            Key::Ctrl(b'U') => {
                self.row.remove_chars(self.cx_min, self.cx);
                self.cx = self.cx_min;
                self.rx = self.row.cx_to_rx[self.cx];
            }
            Key::Ascii(ch) => {
                self.row.insert_char(self.cx, ch);
                self.cx += 1;
                self.rx = self.row.cx_to_rx[self.cx];
            }
            _ => (),
        }
        self.scroll();
    }

    fn scroll(&mut self) {
        if self.rx < self.coloff {
            self.coloff = self.rx;
        }
        if self.rx >= self.coloff + self.width {
            self.coloff = self.rx - self.width + 1;
        }
    }
}
