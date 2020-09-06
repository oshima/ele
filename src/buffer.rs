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
    redraw_cy: Option<usize>,
    saved_rx: usize,
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
            redraw_cy: Some(0),
            saved_rx: 0,
            rowoff: 0,
            coloff: 0,
            rows: Vec::new(),
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
                let string = buf.trim_end_matches(crlf).to_string();
                self.rows.push(Row::new(string));
                ends_with_lf = buf.ends_with("\n");
                buf.clear();
            }
            if self.rows.is_empty() || ends_with_lf {
                self.rows.push(Row::new(String::new()));
            }
        } else {
            self.rows.push(Row::new(String::new()));
        }
        Ok(())
    }

    pub fn set_position(&mut self, x: usize, y: usize, width: usize, height: usize) {
        self.x = x;
        self.y = y;
        self.width = width;
        self.height = height - 1; // subtract status bar
    }

    pub fn draw(&mut self, canvas: &mut Vec<u8>) -> io::Result<()> {
        if let Some(cy) = self.redraw_cy {
            self.draw_rows(cy, canvas)?;
        }
        self.draw_status_bar(canvas)?;
        Ok(())
    }

    fn draw_rows(&mut self, from_cy: usize, canvas: &mut Vec<u8>) -> io::Result<()> {
        canvas.write(
            format!(
                "\x1b[{};{}H",
                self.y + from_cy - self.rowoff + 1,
                self.x + 1
            )
            .as_bytes(),
        )?;

        for y in from_cy..(self.rowoff + self.height) {
            if y < self.rows.len() {
                self.rows[y].draw(self.coloff, self.width, canvas)?;
            }
            canvas.write(b"\x1b[K")?;
            canvas.write(b"\r\n")?;
        }
        Ok(())
    }

    fn draw_status_bar(&mut self, canvas: &mut Vec<u8>) -> io::Result<()> {
        let left = format!(
            "{} {}",
            self.filename.as_deref().unwrap_or("*newfile*"),
            if self.modified { "(modified)" } else { "" },
        );
        let right = format!("Ln {}, Col {}", self.cy + 1, self.rx + 1);
        let right_len = cmp::min(right.len(), self.width);
        let left_len = cmp::min(left.len(), self.width - right_len);
        let padding = self.width - left_len - right_len;

        canvas.write(format!("\x1b[{};{}H", self.height + 1, self.x + 1).as_bytes())?;
        canvas.write(b"\x1b[7m")?;

        canvas.write(&left.as_bytes()[0..left_len])?;
        for _ in 0..padding {
            canvas.write(b" ")?;
        }
        canvas.write(&right.as_bytes()[0..right_len])?;

        canvas.write(b"\x1b[m")?;
        Ok(())
    }

    pub fn draw_cursor(&mut self, canvas: &mut Vec<u8>) -> io::Result<()> {
        canvas.write(
            format!(
                "\x1b[{};{}H",
                self.y + self.cy - self.rowoff + 1,
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
                    self.rx = self.rows[self.cy].cx_to_rx.get(self.cx);
                    self.saved_rx = self.rx;
                } else if self.cy > 0 {
                    self.cy -= 1;
                    self.cx = self.rows[self.cy].max_cx();
                    self.rx = self.rows[self.cy].cx_to_rx.get(self.cx);
                    self.saved_rx = self.rx;
                }
                self.redraw_cy = None;
            }
            Key::ArrowRight | Key::Ctrl(b'F') => {
                if self.cx < self.rows[self.cy].max_cx() {
                    self.cx += 1;
                    self.rx = self.rows[self.cy].cx_to_rx.get(self.cx);
                    self.saved_rx = self.rx;
                } else if self.cy < self.rows.len() - 1 {
                    self.cy += 1;
                    self.cx = 0;
                    self.rx = 0;
                    self.saved_rx = 0;
                }
                self.redraw_cy = None;
            }
            Key::ArrowUp | Key::Ctrl(b'P') => {
                if self.cy > 0 {
                    self.cy -= 1;
                    self.rx = cmp::min(self.saved_rx, self.rows[self.cy].max_rx());
                    self.cx = self.rows[self.cy].rx_to_cx.get(self.rx);
                    self.rx = self.rows[self.cy].cx_to_rx.get(self.cx);
                }
                self.redraw_cy = None;
            }
            Key::ArrowDown | Key::Ctrl(b'N') => {
                if self.cy < self.rows.len() - 1 {
                    self.cy += 1;
                    self.rx = cmp::min(self.saved_rx, self.rows[self.cy].max_rx());
                    self.cx = self.rows[self.cy].rx_to_cx.get(self.rx);
                    self.rx = self.rows[self.cy].cx_to_rx.get(self.cx);
                }
                self.redraw_cy = None;
            }
            Key::Home | Key::Ctrl(b'A') => {
                self.cx = 0;
                self.rx = 0;
                self.saved_rx = 0;
                self.redraw_cy = None;
            }
            Key::End | Key::Ctrl(b'E') => {
                self.cx = self.rows[self.cy].max_cx();
                self.rx = self.rows[self.cy].cx_to_rx.get(self.cx);
                self.saved_rx = self.rx;
                self.redraw_cy = None;
            }
            Key::PageUp | Key::Alt(b'v') => {
                self.cy -= cmp::min(self.height, self.rowoff);
                self.rowoff -= cmp::min(self.height, self.rowoff);
                self.rx = cmp::min(self.saved_rx, self.rows[self.cy].max_rx());
                self.cx = self.rows[self.cy].rx_to_cx.get(self.rx);
                self.rx = self.rows[self.cy].cx_to_rx.get(self.cx);
                self.redraw_cy = Some(self.rowoff);
            }
            Key::PageDown | Key::Ctrl(b'V') => {
                if self.rowoff + self.height < self.rows.len() {
                    self.cy += cmp::min(self.height, self.rows.len() - 1 - self.cy);
                    self.rowoff += self.height;
                    self.rx = cmp::min(self.saved_rx, self.rows[self.cy].max_rx());
                    self.cx = self.rows[self.cy].rx_to_cx.get(self.rx);
                    self.rx = self.rows[self.cy].cx_to_rx.get(self.cx);
                    self.redraw_cy = Some(self.rowoff);
                }
            }
            Key::Backspace | Key::Ctrl(b'H') => {
                if self.cx > 0 {
                    self.rows[self.cy].remove(self.cx - 1);
                    self.cx -= 1;
                    self.rx = self.rows[self.cy].cx_to_rx.get(self.cx);
                    self.saved_rx = self.rx;
                    self.modified = true;
                } else if self.cy > 0 {
                    let row = self.rows.remove(self.cy);
                    let max_cx = self.rows[self.cy - 1].max_cx();
                    self.rows[self.cy - 1].push_str(&row.string);
                    self.cy -= 1;
                    self.cx = max_cx;
                    self.rx = self.rows[self.cy].cx_to_rx.get(self.cx);
                    self.saved_rx = self.rx;
                    self.modified = true;
                }
                self.redraw_cy = Some(self.cy);
            }
            Key::Delete | Key::Ctrl(b'D') => {
                if self.cx < self.rows[self.cy].max_cx() {
                    self.rows[self.cy].remove(self.cx);
                    self.modified = true;
                } else if self.cy < self.rows.len() - 1 {
                    let row = self.rows.remove(self.cy + 1);
                    self.rows[self.cy].push_str(&row.string);
                    self.modified = true;
                }
                self.redraw_cy = Some(self.cy);
            }
            Key::Ctrl(b'I') => {
                self.rows[self.cy].insert(self.cx, '\t');
                self.cx += 1;
                self.rx = self.rows[self.cy].cx_to_rx.get(self.cx);
                self.saved_rx = self.rx;
                self.modified = true;
                self.redraw_cy = Some(self.cy);
            }
            Key::Ctrl(b'J') | Key::Ctrl(b'M') => {
                let string = self.rows[self.cy].split_off(self.cx);
                self.rows.insert(self.cy + 1, Row::new(string));
                self.cy += 1;
                self.cx = 0;
                self.rx = 0;
                self.saved_rx = 0;
                self.modified = true;
                self.redraw_cy = Some(self.cy - 1);
            }
            Key::Ctrl(b'K') => {
                self.rows[self.cy].truncate(self.cx);
                self.redraw_cy = Some(self.cy);
            }
            Key::Ctrl(b'U') => {
                self.rows[self.cy].remove_str(0, self.cx);
                self.cx = 0;
                self.rx = 0;
                self.saved_rx = 0;
                self.redraw_cy = Some(self.cy);
            }
            Key::Alt(b'<') => {
                self.cy = 0;
                self.cx = 0;
                self.rx = 0;
                self.saved_rx = 0;
                self.redraw_cy = None;
            }
            Key::Alt(b'>') => {
                self.cy = self.rows.len() - 1;
                self.cx = self.rows[self.cy].max_cx();
                self.rx = self.rows[self.cy].cx_to_rx.get(self.cx);
                self.saved_rx = self.rx;
                self.redraw_cy = None;
            }
            Key::Char(ch) => {
                self.rows[self.cy].insert(self.cx, ch);
                self.cx += 1;
                self.rx = self.rows[self.cy].cx_to_rx.get(self.cx);
                self.saved_rx = self.rx;
                self.modified = true;
                self.redraw_cy = Some(self.cy);
            }
            _ => (),
        }
        self.scroll();
    }

    fn scroll(&mut self) {
        if self.cy < self.rowoff {
            self.rowoff = self.cy;
            self.redraw_cy = Some(self.rowoff);
        }
        if self.cy >= self.rowoff + self.height {
            self.rowoff = self.cy - self.height + 1;
            self.redraw_cy = Some(self.rowoff);
        }
        if self.rx < self.coloff {
            self.coloff = self.rx;
            self.redraw_cy = Some(self.rowoff);
        }
        if self.rx >= self.coloff + self.width {
            self.coloff = self.rx - self.width + 1;
            self.redraw_cy = Some(self.rowoff);

            let row = &self.rows[self.cy];
            if self.rx < row.max_rx()
                && row.rx_to_idx.get(self.rx) == row.rx_to_idx.get(self.rx + 1)
            {
                self.coloff += 1;
            }
        }
    }

    pub fn save(&mut self) -> io::Result<()> {
        if let Some(filename) = &self.filename {
            let file = File::create(filename)?;
            let mut writer = BufWriter::new(file);

            for (i, row) in self.rows.iter().enumerate() {
                writer.write(row.string.as_bytes())?;
                if i < self.rows.len() - 1 {
                    writer.write(b"\n")?;
                }
            }
            self.modified = false;
        }
        Ok(())
    }
}
