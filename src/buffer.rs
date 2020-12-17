use std::cmp;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::ops::Range;

use crate::canvas::Canvas;
use crate::coord::{Cursor, Pos, Size};
use crate::face::Face;
use crate::key::Key;
use crate::row::Row;
use crate::syntax::Syntax;

enum Redraw {
    None,
    Min,
    End,
    Whole,
}

#[derive(Default)]
struct Search {
    matches: Vec<Match>,
    match_idx: usize,
    orig_offset: Pos,
    orig_cursor: Cursor,
}

struct Match {
    pos: Pos,
    faces: Vec<Face>,
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
    search: Search,
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
            search: Default::default(),
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

    pub fn draw(&mut self, canvas: &mut Canvas) -> io::Result<()> {
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

    fn draw_rows(&mut self, canvas: &mut Canvas, y_range: Range<usize>) -> io::Result<()> {
        canvas.write(
            format!(
                "\x1b[{};{}H",
                self.pos.y + y_range.start - self.offset.y + 1,
                self.pos.x + 1
            )
            .as_bytes(),
        )?;
        canvas.set_color(Face::Background)?;

        for y in y_range {
            if y < self.rows.len() {
                let x_range = self.offset.x..(self.offset.x + self.size.w);
                self.rows[y].draw(canvas, x_range)?;
            }
            canvas.write(b"\x1b[K")?;
            canvas.write(b"\r\n")?;
        }

        canvas.reset_color()?;
        Ok(())
    }

    fn draw_status_bar(&self, canvas: &mut Canvas) -> io::Result<()> {
        let filename = format!(
            " {}{} ",
            self.filename.as_deref().unwrap_or("newfile"),
            if self.modified { " +" } else { "" },
        );
        let cursor = format!(" {}, {} ", self.cursor.y + 1, self.cursor.x + 1);
        let syntax = format!(" {} ", self.syntax.name());
        let padding = self
            .size
            .w
            .saturating_sub(filename.len() + cursor.len() + syntax.len());

        canvas.write(
            format!("\x1b[{};{}H", self.pos.y + self.size.h + 1, self.pos.x + 1).as_bytes(),
        )?;
        canvas.set_color(Face::StatusBar)?;
        canvas.set_color(Face::Default)?;

        canvas.write(filename.as_bytes())?;
        for _ in 0..padding {
            canvas.write(b" ")?;
        }
        canvas.write(cursor.as_bytes())?;

        canvas.write(self.syntax.color(canvas.term))?;
        canvas.write(syntax.as_bytes())?;

        canvas.reset_color()?;
        Ok(())
    }

    pub fn draw_cursor(&self, canvas: &mut Canvas) -> io::Result<()> {
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

impl Buffer {
    pub fn search(&mut self, query: &str, backward: bool) {
        for (y, row) in self.rows.iter_mut().enumerate() {
            for (idx, _) in row.string.match_indices(query) {
                let pos = Pos::new(row.idx_to_x(idx), y);
                let mut faces = Vec::new();
                faces.resize(query.len(), Face::Match);
                faces.swap_with_slice(&mut row.faces[idx..(idx + query.len())]);
                self.search.matches.push(Match { pos, faces });
            }
        }

        if self.search.matches.is_empty() {
            return;
        }

        let mut matches = self.search.matches.iter();
        let cursor_pos = self.cursor.as_pos();
        self.search.match_idx = if backward {
            matches
                .rposition(|m| m.pos < cursor_pos)
                .unwrap_or(self.search.matches.len() - 1)
        } else {
            matches.position(|m| m.pos >= cursor_pos).unwrap_or(0)
        };

        self.search.orig_offset = self.offset;
        self.search.orig_cursor = self.cursor;

        self.move_to_match();
        self.highlight_match(Face::CurrentMatch);
        self.redraw = Redraw::Whole;
    }

    pub fn next_match(&mut self, backward: bool) {
        if self.search.matches.len() <= 1 {
            return;
        }

        self.highlight_match(Face::Match);

        self.search.match_idx = if backward {
            if self.search.match_idx > 0 {
                self.search.match_idx - 1
            } else {
                self.search.matches.len() - 1
            }
        } else {
            if self.search.match_idx < self.search.matches.len() - 1 {
                self.search.match_idx + 1
            } else {
                0
            }
        };

        self.move_to_match();
        self.highlight_match(Face::CurrentMatch);
        self.redraw = Redraw::Whole;
    }

    pub fn clear_matches(&mut self, restore: bool) {
        if self.search.matches.is_empty() {
            return;
        }

        for mat in self.search.matches.iter_mut() {
            let row = &mut self.rows[mat.pos.y];
            let idx = row.x_to_idx(mat.pos.x);
            row.faces[idx..(idx + mat.faces.len())].swap_with_slice(&mut mat.faces);
        }

        self.search.matches.clear();
        if restore {
            self.offset = self.search.orig_offset;
            self.cursor = self.search.orig_cursor;
        }
        self.redraw = Redraw::Whole;
    }

    fn move_to_match(&mut self) {
        let mat = &self.search.matches[self.search.match_idx];

        if mat.pos.x < self.offset.x || mat.pos.x >= self.offset.x + self.size.w {
            self.offset.x = mat.pos.x.saturating_sub(self.size.w / 2);
        }
        if mat.pos.y < self.offset.y || mat.pos.y >= self.offset.y + self.size.h {
            self.offset.y = mat.pos.y.saturating_sub(self.size.h / 2);
        }
        self.cursor.x = mat.pos.x;
        self.cursor.y = mat.pos.y;
        self.cursor.last_x = mat.pos.x;
    }

    fn highlight_match(&mut self, face: Face) {
        let mat = &self.search.matches[self.search.match_idx];
        let row = &mut self.rows[mat.pos.y];
        let idx = row.x_to_idx(mat.pos.x);

        for i in idx..(idx + mat.faces.len()) {
            row.faces[i] = face;
        }
    }
}
