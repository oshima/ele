use std::io::{self, Write};

use crate::canvas::Canvas;
use crate::coord::{Pos, Size};
use crate::face::{Bg, Fg};
use crate::key::Key;
use crate::row::Row;

pub struct Minibuffer {
    pos: Pos,
    size: Size,
    offset: usize,
    cursor: usize,
    prompt_len: usize,
    row: Row,
    draw: bool,
}

impl Minibuffer {
    pub fn new() -> Self {
        Self {
            pos: Pos::new(0, 0),
            size: Size::new(0, 0),
            offset: 0,
            cursor: 0,
            prompt_len: 0,
            row: Row::new(String::new()),
            draw: true,
        }
    }

    pub fn set_message(&mut self, string: &str) {
        self.row.clear();
        self.row.push_str(string);
        self.offset = 0;
        self.cursor = 0;
        self.prompt_len = 0;
        self.highlight();
    }

    pub fn set_prompt(&mut self, string: &str) {
        self.row.clear();
        self.row.push_str(string);
        self.offset = 0;
        self.cursor = self.row.max_x();
        self.prompt_len = self.row.max_x();
        self.highlight();
    }

    pub fn get_input(&self) -> String {
        self.row.string[self.prompt_len..].to_string()
    }

    pub fn resize(&mut self, pos: Pos, size: Size) {
        self.pos = pos;
        self.size = size;
        self.scroll();
        self.draw = true;
    }

    pub fn draw(&mut self, canvas: &mut Canvas) -> io::Result<()> {
        if !self.draw {
            return Ok(());
        }

        write!(canvas, "\x1b[{};{}H", self.pos.y + 1, self.pos.x + 1)?;

        let x_range = self.offset..(self.offset + self.size.w);
        self.row.draw(canvas, x_range)?;

        canvas.write(b"\x1b[K")?;

        self.draw = false;
        Ok(())
    }

    pub fn draw_cursor(&self, canvas: &mut Canvas) -> io::Result<()> {
        write!(
            canvas,
            "\x1b[{};{}H",
            self.pos.y + 1,
            self.pos.x + self.cursor - self.offset + 1,
        )
    }

    pub fn process_keypress(&mut self, key: Key) {
        match key {
            Key::ArrowLeft | Key::Ctrl(b'B') => {
                if let Some(x) = self.row.prev_x(self.cursor) {
                    self.cursor = x;
                    self.scroll();
                }
            }
            Key::ArrowRight | Key::Ctrl(b'F') => {
                if let Some(x) = self.row.next_x(self.cursor) {
                    self.cursor = x;
                    self.scroll();
                }
            }
            Key::Home | Key::Ctrl(b'A') => {
                self.cursor = if self.cursor == self.prompt_len {
                    0
                } else {
                    self.prompt_len
                };
                self.scroll();
            }
            Key::End | Key::Ctrl(b'E') => {
                self.cursor = self.row.max_x();
                self.scroll();
            }
            Key::Backspace | Key::Ctrl(b'H') => {
                if self.cursor > self.prompt_len {
                    if let Some(x) = self.row.prev_x(self.cursor) {
                        self.row.remove_str(x, self.cursor);
                        self.cursor = x;
                        self.highlight();
                        self.scroll();
                    }
                }
            }
            Key::Delete | Key::Ctrl(b'D') => {
                if self.cursor >= self.prompt_len {
                    if let Some(x) = self.row.next_x(self.cursor) {
                        self.row.remove_str(self.cursor, x);
                        self.highlight();
                    }
                }
            }
            Key::Ctrl(b'I') => {
                if self.cursor >= self.prompt_len {
                    let x = self.row.insert_str(self.cursor, "\t");
                    self.cursor = x;
                    self.highlight();
                    self.scroll();
                }
            }
            Key::Ctrl(b'K') => {
                if self.cursor >= self.prompt_len {
                    self.row.truncate(self.cursor);
                    self.highlight();
                }
            }
            Key::Ctrl(b'U') => {
                if self.cursor > self.prompt_len {
                    self.row.remove_str(self.prompt_len, self.cursor);
                    self.cursor = self.prompt_len;
                    self.highlight();
                    self.scroll();
                }
            }
            Key::Char(ch) => {
                if self.cursor >= self.prompt_len {
                    let x = self.row.insert_str(self.cursor, &ch.to_string());
                    self.cursor = x;
                    self.highlight();
                    self.scroll();
                }
            }
            _ => (),
        }
    }

    fn highlight(&mut self) {
        self.row.faces.clear();
        self.row
            .faces
            .resize(self.prompt_len, (Fg::Prompt, Bg::Default));
        self.row
            .faces
            .resize(self.row.string.len(), (Fg::Default, Bg::Default));
        self.draw = true;
    }

    fn scroll(&mut self) {
        if self.cursor < self.offset {
            self.offset = self.cursor;
            self.draw = true;
        }
        if self.cursor >= self.offset + self.size.w {
            self.offset = self.cursor - self.size.w + 1;
            self.draw = true;
        }
    }
}
