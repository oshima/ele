use std::io::{self, Write};

use crate::key::Key;
use crate::row::Row;
use crate::syntax::Hl;

pub struct Minibuffer {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    prompt_len: usize,
    cx: usize,
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
            prompt_len: 0,
            cx: 0,
            rx: 0,
            coloff: 0,
            row: Row::new(String::new()),
        }
    }

    pub fn locate(&mut self, x: usize, y: usize, width: usize, height: usize) {
        self.x = x;
        self.y = y;
        self.width = width;
        self.height = height;
    }

    pub fn set_message(&mut self, string: &str) {
        self.row.clear();
        self.row.push_str(string);
        self.prompt_len = 0;
        self.cx = 0;
        self.rx = 0;
        self.coloff = 0;
    }

    pub fn set_prompt(&mut self, string: &str) {
        self.row.clear();
        self.row.push_str(string);
        self.prompt_len = self.row.max_cx();
        self.cx = self.row.max_cx();
        self.rx = self.row.max_rx();
        self.coloff = 0;
    }

    pub fn get_input(&self) -> String {
        self.row.string[self.prompt_len..].to_string()
    }

    pub fn draw(&mut self, canvas: &mut Vec<u8>) -> io::Result<()> {
        canvas.write(format!("\x1b[{};{}H", self.y + 1, self.x + 1).as_bytes())?;

        self.row.hls.clear();
        self.row.hls.resize(self.row.render.len(), Hl::Default);
        for i in 0..self.row.cx_to_rx.get(self.prompt_len) {
            self.row.hls[i] = Hl::Function;
        }

        self.row.draw(canvas, self.coloff, self.width)?;

        canvas.write(b"\x1b[K")?;
        Ok(())
    }

    pub fn draw_cursor(&self, canvas: &mut Vec<u8>) -> io::Result<()> {
        canvas.write(
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
                if self.cx > 0 {
                    self.cx -= 1;
                    self.rx = self.row.cx_to_rx.get(self.cx);
                }
            }
            Key::ArrowRight | Key::Ctrl(b'F') => {
                if self.cx < self.row.max_cx() {
                    self.cx += 1;
                    self.rx = self.row.cx_to_rx.get(self.cx);
                }
            }
            Key::Home | Key::Ctrl(b'A') | Key::Alt(b'<') => {
                self.cx = self.prompt_len;
                self.rx = self.row.cx_to_rx.get(self.cx);
                self.coloff = 0;
            }
            Key::End | Key::Ctrl(b'E') | Key::Alt(b'>') => {
                self.cx = self.row.max_cx();
                self.rx = self.row.max_rx();
            }
            Key::Backspace | Key::Ctrl(b'H') => {
                if self.cx > self.prompt_len {
                    self.row.remove(self.cx - 1);
                    self.cx -= 1;
                    self.rx = self.row.cx_to_rx.get(self.cx);
                }
            }
            Key::Delete | Key::Ctrl(b'D') => {
                if self.cx >= self.prompt_len && self.cx < self.row.max_cx() {
                    self.row.remove(self.cx);
                }
            }
            Key::Ctrl(b'I') => {
                if self.cx >= self.prompt_len {
                    self.row.insert(self.cx, '\t');
                    self.cx += 1;
                    self.rx = self.row.cx_to_rx.get(self.cx);
                }
            }
            Key::Ctrl(b'K') => {
                if self.cx >= self.prompt_len {
                    self.row.truncate(self.cx);
                }
            }
            Key::Ctrl(b'U') => {
                if self.cx > self.prompt_len {
                    self.row.remove_str(self.prompt_len, self.cx);
                    self.cx = self.prompt_len;
                    self.rx = self.row.cx_to_rx.get(self.cx);
                    self.coloff = 0;
                }
            }
            Key::Char(ch) => {
                if self.cx >= self.prompt_len {
                    self.row.insert(self.cx, ch);
                    self.cx += 1;
                    self.rx = self.row.cx_to_rx.get(self.cx);
                }
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

            if self.rx < self.row.max_rx()
                && self.row.rx_to_idx.get(self.rx) == self.row.rx_to_idx.get(self.rx + 1)
            {
                self.coloff += 1;
            }
        }
    }
}
