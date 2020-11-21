use std::cmp;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::ops::Range;

use crate::coord::{Cursor, Pos, Size};
use crate::key::Key;
use crate::row::Row;
use crate::syntax::Syntax;

enum Redraw {
    None,
    Min,
    End,
    Whole,
}

pub struct Buffer {
    pub filename: Option<String>,
    pub modified: bool,
    pub pos: Pos,
    pub size: Size,
    offset: Pos,
    cursor: Cursor,
    hl_from: Option<usize>,
    redraw: Redraw,
    rows: Vec<Row>,
    syntax: Box<dyn Syntax>,
}

impl Buffer {
    pub fn new(filename: Option<String>) -> io::Result<Self> {
        let mut buffer = Self {
            syntax: Syntax::detect(&filename),
            filename,
            modified: false,
            pos: Pos::new(0, 0),
            size: Size::new(0, 0),
            offset: Pos::new(0, 0),
            cursor: Cursor::new(0, 0),
            hl_from: Some(0),
            redraw: Redraw::Whole,
            rows: Vec::new(),
        };
        buffer.init()?;
        Ok(buffer)
    }

    fn init(&mut self) -> io::Result<()> {
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
            for row in self.rows.iter_mut() {
                row.hl_context = 0;
            }
            self.modified = false;
            self.hl_from = Some(0);
            self.redraw = Redraw::Whole;
        }
        self.syntax = Syntax::detect(&self.filename);
        Ok(())
    }

    pub fn draw(&mut self, canvas: &mut Vec<u8>) -> io::Result<()> {
        if let Some(y) = self.hl_from {
            let n_updates = self.syntax.highlight(&mut self.rows[y..]);
            let y_range = match self.redraw {
                Redraw::None => y..y,
                Redraw::Min => y..cmp::min(y + n_updates, self.offset.y + self.size.h),
                Redraw::End => y..(self.offset.y + self.size.h),
                Redraw::Whole => self.offset.y..(self.offset.y + self.size.h),
            };
            self.draw_rows(canvas, y_range)?;
        } else if let Redraw::Whole = self.redraw {
            let y_range = self.offset.y..(self.offset.y + self.size.h);
            self.draw_rows(canvas, y_range)?;
        }

        self.draw_status_bar(canvas)?;
        Ok(())
    }

    fn draw_rows(&mut self, canvas: &mut Vec<u8>, y_range: Range<usize>) -> io::Result<()> {
        canvas.write(
            format!(
                "\x1b[{};{}H",
                self.pos.y + y_range.start - self.offset.y + 1,
                self.pos.x + 1
            )
            .as_bytes(),
        )?;

        for y in y_range {
            if y < self.rows.len() {
                let x_range = self.offset.x..(self.offset.x + self.size.w);
                self.rows[y].draw(canvas, x_range)?;
            }
            canvas.write(b"\x1b[K")?;
            canvas.write(b"\r\n")?;
        }
        Ok(())
    }

    fn draw_status_bar(&self, canvas: &mut Vec<u8>) -> io::Result<()> {
        let left = format!(
            "{} {}",
            self.filename.as_deref().unwrap_or("*newfile*"),
            if self.modified { "(modified)" } else { "" },
        );
        let right = format!(
            "Ln {}, Col {} {}",
            self.cursor.y + 1,
            self.cursor.x + 1,
            self.syntax.name(),
        );
        let right_len = cmp::min(right.len(), self.size.w);
        let left_len = cmp::min(left.len(), self.size.w - right_len);
        let padding = self.size.w - left_len - right_len;

        canvas.write(
            format!("\x1b[{};{}H", self.pos.y + self.size.h + 1, self.pos.x + 1).as_bytes(),
        )?;
        canvas.write(b"\x1b[7m")?;

        canvas.write(&left.as_bytes()[0..left_len])?;
        for _ in 0..padding {
            canvas.write(b" ")?;
        }
        canvas.write(&right.as_bytes()[0..right_len])?;

        canvas.write(b"\x1b[m")?;
        Ok(())
    }

    pub fn draw_cursor(&self, canvas: &mut Vec<u8>) -> io::Result<()> {
        canvas.write(
            format!(
                "\x1b[{};{}H",
                self.pos.y + self.cursor.y - self.offset.y + 1,
                self.pos.x + self.cursor.x - self.offset.x + 1,
            )
            .as_bytes(),
        )?;
        Ok(())
    }

    pub fn process_keypress(&mut self, key: Key) {
        match key {
            Key::ArrowLeft | Key::Ctrl(b'B') => {
                if self.cursor.x > 0 {
                    self.cursor.x = self.rows[self.cursor.y].prev_x(self.cursor.x);
                    self.cursor.last_x = self.cursor.x;
                } else if self.cursor.y > 0 {
                    self.cursor.y -= 1;
                    self.cursor.x = self.rows[self.cursor.y].max_x();
                    self.cursor.last_x = self.cursor.x;
                }
                self.hl_from = None;
                self.redraw = Redraw::None;
            }
            Key::ArrowRight | Key::Ctrl(b'F') => {
                if self.cursor.x < self.rows[self.cursor.y].max_x() {
                    self.cursor.x = self.rows[self.cursor.y].next_x(self.cursor.x);
                    self.cursor.last_x = self.cursor.x;
                } else if self.cursor.y < self.rows.len() - 1 {
                    self.cursor.y += 1;
                    self.cursor.x = 0;
                    self.cursor.last_x = self.cursor.x;
                }
                self.hl_from = None;
                self.redraw = Redraw::None;
            }
            Key::ArrowUp | Key::Ctrl(b'P') => {
                if self.cursor.y > 0 {
                    self.cursor.y -= 1;
                    self.cursor.x = self.rows[self.cursor.y].prev_fit_x(self.cursor.last_x);
                }
                self.hl_from = None;
                self.redraw = Redraw::None;
            }
            Key::ArrowDown | Key::Ctrl(b'N') => {
                if self.cursor.y < self.rows.len() - 1 {
                    self.cursor.y += 1;
                    self.cursor.x = self.rows[self.cursor.y].prev_fit_x(self.cursor.last_x);
                }
                self.hl_from = None;
                self.redraw = Redraw::None;
            }
            Key::Home | Key::Ctrl(b'A') => {
                let x = self.rows[self.cursor.y].first_letter_x();
                self.cursor.x = if self.cursor.x == x { 0 } else { x };
                self.cursor.last_x = self.cursor.x;
                self.hl_from = None;
                self.redraw = Redraw::None;
            }
            Key::End | Key::Ctrl(b'E') => {
                self.cursor.x = self.rows[self.cursor.y].max_x();
                self.cursor.last_x = self.cursor.x;
                self.hl_from = None;
                self.redraw = Redraw::None;
            }
            Key::PageUp | Key::Alt(b'v') => {
                if self.offset.y > 0 {
                    self.cursor.y -= cmp::min(self.size.h, self.offset.y);
                    self.offset.y -= cmp::min(self.size.h, self.offset.y);
                    self.cursor.x = self.rows[self.cursor.y].prev_fit_x(self.cursor.last_x);
                    self.hl_from = None;
                    self.redraw = Redraw::Whole;
                } else {
                    self.hl_from = None;
                    self.redraw = Redraw::None;
                }
            }
            Key::PageDown | Key::Ctrl(b'V') => {
                if self.offset.y + self.size.h < self.rows.len() {
                    self.cursor.y += cmp::min(self.size.h, self.rows.len() - self.cursor.y - 1);
                    self.offset.y += self.size.h;
                    self.cursor.x = self.rows[self.cursor.y].prev_fit_x(self.cursor.last_x);
                    self.hl_from = None;
                    self.redraw = Redraw::Whole;
                } else {
                    self.hl_from = None;
                    self.redraw = Redraw::None;
                }
            }
            Key::Backspace | Key::Ctrl(b'H') => {
                if self.cursor.x > 0 {
                    self.cursor.x = self.rows[self.cursor.y].prev_x(self.cursor.x);
                    self.cursor.last_x = self.cursor.x;
                    self.rows[self.cursor.y].remove(self.cursor.x);
                    self.modified = true;
                    self.hl_from = Some(self.cursor.y);
                    self.redraw = Redraw::Min;
                } else if self.cursor.y > 0 {
                    let row = self.rows.remove(self.cursor.y);
                    self.cursor.y -= 1;
                    self.cursor.x = self.rows[self.cursor.y].max_x();
                    self.cursor.last_x = self.cursor.x;
                    self.rows[self.cursor.y].push_str(&row.string);
                    self.modified = true;
                    self.hl_from = Some(self.cursor.y);
                    self.redraw = Redraw::End;
                } else {
                    self.hl_from = None;
                    self.redraw = Redraw::None;
                }
            }
            Key::Delete | Key::Ctrl(b'D') => {
                if self.cursor.x < self.rows[self.cursor.y].max_x() {
                    self.rows[self.cursor.y].remove(self.cursor.x);
                    self.modified = true;
                    self.hl_from = Some(self.cursor.y);
                    self.redraw = Redraw::Min;
                } else if self.cursor.y < self.rows.len() - 1 {
                    let row = self.rows.remove(self.cursor.y + 1);
                    self.rows[self.cursor.y].push_str(&row.string);
                    self.modified = true;
                    self.hl_from = Some(self.cursor.y);
                    self.redraw = Redraw::End;
                } else {
                    self.hl_from = None;
                    self.redraw = Redraw::None;
                }
            }
            Key::Ctrl(b'I') => {
                self.rows[self.cursor.y].insert(self.cursor.x, '\t');
                self.cursor.x = self.rows[self.cursor.y].next_x(self.cursor.x);
                self.cursor.last_x = self.cursor.x;
                self.modified = true;
                self.hl_from = Some(self.cursor.y);
                self.redraw = Redraw::Min;
            }
            Key::Ctrl(b'J') | Key::Ctrl(b'M') => {
                let string = self.rows[self.cursor.y].split_off(self.cursor.x);
                self.rows.insert(self.cursor.y + 1, Row::new(string));
                self.cursor.y += 1;
                self.cursor.x = 0;
                self.cursor.last_x = self.cursor.x;
                self.modified = true;
                self.hl_from = Some(self.cursor.y - 1);
                self.redraw = Redraw::End;
            }
            Key::Ctrl(b'K') => {
                self.rows[self.cursor.y].truncate(self.cursor.x);
                self.modified = true;
                self.hl_from = Some(self.cursor.y);
                self.redraw = Redraw::Min;
            }
            Key::Ctrl(b'U') => {
                self.rows[self.cursor.y].remove_str(0, self.cursor.x);
                self.cursor.x = 0;
                self.cursor.last_x = self.cursor.x;
                self.modified = true;
                self.hl_from = Some(self.cursor.y);
                self.redraw = Redraw::Min;
            }
            Key::Alt(b'<') => {
                self.cursor.y = 0;
                self.cursor.x = 0;
                self.cursor.last_x = self.cursor.x;
                self.hl_from = None;
                self.redraw = Redraw::None;
            }
            Key::Alt(b'>') => {
                self.cursor.y = self.rows.len() - 1;
                self.cursor.x = self.rows[self.cursor.y].max_x();
                self.cursor.last_x = self.cursor.x;
                self.hl_from = None;
                self.redraw = Redraw::None;
            }
            Key::Char(ch) => {
                self.rows[self.cursor.y].insert(self.cursor.x, ch);
                self.cursor.x = self.rows[self.cursor.y].next_x(self.cursor.x);
                self.cursor.last_x = self.cursor.x;
                self.modified = true;
                self.hl_from = Some(self.cursor.y);
                self.redraw = Redraw::Min;
            }
            _ => (),
        }
        self.scroll();
    }

    fn scroll(&mut self) {
        if self.cursor.y < self.offset.y {
            self.offset.y = self.cursor.y;
            self.redraw = Redraw::Whole;
        }
        if self.cursor.y >= self.offset.y + self.size.h {
            self.offset.y = self.cursor.y - self.size.h + 1;
            self.redraw = Redraw::Whole;
        }
        if self.cursor.x < self.offset.x {
            self.offset.x = self.cursor.x;
            self.redraw = Redraw::Whole;
        }
        if self.cursor.x >= self.offset.x + self.size.w {
            self.offset.x = self.cursor.x - self.size.w + 1;
            self.redraw = Redraw::Whole;
        }
    }
}
