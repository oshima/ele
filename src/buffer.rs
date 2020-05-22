use std::cmp;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};

use crate::key::Key;
use crate::row::Row;

pub struct Buffer {
    pub filename: Option<String>,
    pub modified: bool,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    cx: usize,
    cy: usize,
    rx: usize,
    rx_cache: usize,
    rowoff: usize,
    coloff: usize,
    rows: Vec<Row>,
}

impl Buffer {
    pub fn new(filename: Option<String>) -> io::Result<Self> {
        let mut buffer = Self {
            filename,
            modified: false,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            cx: 0,
            cy: 0,
            rx: 0,
            rx_cache: 0,
            rowoff: 0,
            coloff: 0,
            rows: vec![],
        };
        buffer.open_file()?;
        Ok(buffer)
    }

    fn open_file(&mut self) -> io::Result<()> {
        if let Some(filename) = &self.filename {
            let file = File::open(filename)?;
            let mut reader = BufReader::new(file);
            let mut buf = String::new();

            let crlf: &[_] = &['\r', '\n'];
            let mut ends_with_lf = false;

            while reader.read_line(&mut buf)? > 0 {
                let chars = buf.trim_end_matches(crlf).as_bytes().to_vec();
                self.rows.push(Row::new(chars));
                ends_with_lf = buf.ends_with("\n");
                buf.clear();
            }
            if self.rows.is_empty() || ends_with_lf {
                self.rows.push(Row::new(vec![]));
            }
        } else {
            self.rows.push(Row::new(vec![]));
        }
        Ok(())
    }

    pub fn set_position(&mut self, x: usize, y: usize, width: usize, height: usize) {
        self.x = x;
        self.y = y;
        self.width = width;
        self.height = height - 1; // subtract status bar
    }

    pub fn draw(&mut self, bufout: &mut Vec<u8>) -> io::Result<()> {
        bufout.write(format!("\x1b[{};{}H", self.y + 1, self.x + 1).as_bytes())?;
        self.draw_rows(bufout)?;
        self.draw_status_bar(bufout)?;
        Ok(())
    }

    fn draw_rows(&mut self, bufout: &mut Vec<u8>) -> io::Result<()> {
        for y in self.rowoff..(self.rowoff + self.height) {
            if y < self.rows.len() {
                let row = &self.rows[y];
                if row.render.len() > self.coloff {
                    let len = cmp::min(row.render.len() - self.coloff, self.width);
                    bufout.write(&row.render[self.coloff..(self.coloff + len)])?;
                }
            }
            bufout.write(b"\x1b[K")?;
            bufout.write(b"\r\n")?;
        }
        Ok(())
    }

    fn draw_status_bar(&mut self, bufout: &mut Vec<u8>) -> io::Result<()> {
        let left = format!(
            "{} {}",
            match &self.filename {
                None => "*newfile*",
                Some(filename) => filename,
            },
            if self.modified { "(modified)" } else { "" },
        );
        let right = format!("Ln {}, Col {}", self.cy + 1, self.rx + 1);
        let right_len = cmp::min(right.len(), self.width);
        let left_len = cmp::min(left.len(), self.width - right_len);
        let padding = self.width - left_len - right_len;

        bufout.write(b"\x1b[7m")?;

        bufout.write(&left.as_bytes()[0..left_len])?;
        for _ in 0..padding {
            bufout.write(b" ")?;
        }
        bufout.write(&right.as_bytes()[0..right_len])?;

        bufout.write(b"\x1b[m")?;
        Ok(())
    }

    pub fn draw_cursor(&mut self, bufout: &mut Vec<u8>) -> io::Result<()> {
        bufout.write(
            format!(
                "\x1b[{};{}H",
                self.y + self.cy - self.rowoff + 1,
                self.x + self.rx - self.coloff + 1,
            )
            .as_bytes(),
        )?;
        Ok(())
    }

    pub fn process_keypress(&mut self, key: Key) -> io::Result<()> {
        match key {
            Key::ArrowLeft | Key::Ctrl(b'b') => {
                if self.cx > 0 {
                    self.cx -= 1;
                    self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                    self.rx_cache = self.rx;
                } else if self.cy > 0 {
                    self.cy -= 1;
                    self.cx = self.rows[self.cy].chars.len();
                    self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                    self.rx_cache = self.rx;
                }
            }
            Key::ArrowRight | Key::Ctrl(b'f') => {
                if self.cx < self.rows[self.cy].chars.len() {
                    self.cx += 1;
                    self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                    self.rx_cache = self.rx;
                } else if self.cy < self.rows.len() - 1 {
                    self.cy += 1;
                    self.cx = 0;
                    self.rx = 0;
                    self.rx_cache = 0;
                }
            }
            Key::ArrowUp | Key::Ctrl(b'p') => {
                if self.cy > 0 {
                    self.cy -= 1;
                    self.rx = cmp::min(self.rx_cache, self.rows[self.cy].render.len());
                    self.cx = self.rows[self.cy].rx_to_cx[self.rx];
                    self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                }
            }
            Key::ArrowDown | Key::Ctrl(b'n') => {
                if self.cy < self.rows.len() - 1 {
                    self.cy += 1;
                    self.rx = cmp::min(self.rx_cache, self.rows[self.cy].render.len());
                    self.cx = self.rows[self.cy].rx_to_cx[self.rx];
                    self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                }
            }
            Key::Home | Key::Ctrl(b'a') => {
                self.cx = 0;
                self.rx = 0;
                self.rx_cache = 0;
            }
            Key::End | Key::Ctrl(b'e') => {
                self.cx = self.rows[self.cy].chars.len();
                self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                self.rx_cache = self.rx;
            }
            Key::PageUp | Key::Alt(b'v') => {
                self.cy -= cmp::min(self.height, self.rowoff);
                self.rowoff -= cmp::min(self.height, self.rowoff);

                self.rx = cmp::min(self.rx_cache, self.rows[self.cy].render.len());
                self.cx = self.rows[self.cy].rx_to_cx[self.rx];
                self.rx = self.rows[self.cy].cx_to_rx[self.cx];
            }
            Key::PageDown | Key::Ctrl(b'v') => {
                if self.rowoff + self.height < self.rows.len() {
                    self.cy += cmp::min(self.height, self.rows.len() - 1 - self.cy);
                    self.rowoff += self.height;

                    self.rx = cmp::min(self.rx_cache, self.rows[self.cy].render.len());
                    self.cx = self.rows[self.cy].rx_to_cx[self.rx];
                    self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                }
            }
            Key::Backspace | Key::Ctrl(b'h') => {
                if self.cx > 0 {
                    self.rows[self.cy].delete_char(self.cx - 1);
                    self.cx -= 1;
                    self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                    self.rx_cache = self.rx;
                    self.modified = true;
                } else if self.cy > 0 {
                    let mut row = self.rows.remove(self.cy);
                    let len = self.rows[self.cy - 1].chars.len();
                    self.rows[self.cy - 1].append_chars(&mut row.chars);
                    self.cy -= 1;
                    self.cx = len;
                    self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                    self.rx_cache = self.rx;
                    self.modified = true;
                }
            }
            Key::Delete | Key::Ctrl(b'd') => {
                if self.cx < self.rows[self.cy].chars.len() {
                    self.rows[self.cy].delete_char(self.cx);
                    self.modified = true;
                } else if self.cy < self.rows.len() - 1 {
                    let mut row = self.rows.remove(self.cy + 1);
                    self.rows[self.cy].append_chars(&mut row.chars);
                    self.modified = true;
                }
            }
            Key::Ctrl(b'i') => {
                self.rows[self.cy].insert_char(self.cx, b'\t');
                self.cx += 1;
                self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                self.rx_cache = self.rx;
                self.modified = true;
            }
            Key::Ctrl(b'j') | Key::Ctrl(b'm') => {
                let chars = self.rows[self.cy].split_chars(self.cx);
                self.rows.insert(self.cy + 1, Row::new(chars));
                self.cy += 1;
                self.cx = 0;
                self.rx = 0;
                self.rx_cache = 0;
                self.modified = true;
            }
            Key::Ctrl(b'k') => {
                self.rows[self.cy].truncate_chars(self.cx);
            }
            Key::Ctrl(b'u') => {
                self.rows[self.cy].remove_chars(0, self.cx);
                self.cx = 0;
                self.rx = 0;
                self.rx_cache = 0;
            }
            Key::Alt(b'<') => {
                self.cy = 0;
                self.cx = 0;
                self.rx = 0;
                self.rx_cache = 0;
            }
            Key::Alt(b'>') => {
                self.cy = self.rows.len() - 1;
                self.cx = self.rows[self.cy].chars.len();
                self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                self.rx_cache = self.rx;
            }
            Key::Plain(ch) => {
                self.rows[self.cy].insert_char(self.cx, ch);
                self.cx += 1;
                self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                self.rx_cache = self.rx;
                self.modified = true;
            }
            _ => (),
        }
        self.scroll();
        Ok(())
    }

    fn scroll(&mut self) {
        if self.cy < self.rowoff {
            self.rowoff = self.cy;
        }
        if self.cy >= self.rowoff + self.height {
            self.rowoff = self.cy - self.height + 1;
        }
        if self.rx < self.coloff {
            self.coloff = self.rx;
        }
        if self.rx >= self.coloff + self.width {
            self.coloff = self.rx - self.width + 1;
        }
    }

    pub fn save(&mut self) -> io::Result<()> {
        if let Some(filename) = &self.filename {
            let file = File::create(filename)?;
            let mut writer = BufWriter::new(file);

            for (i, row) in self.rows.iter().enumerate() {
                writer.write(&row.chars)?;
                if i < self.rows.len() - 1 {
                    writer.write(b"\n")?;
                }
            }
            self.modified = false;
        }
        Ok(())
    }
}
