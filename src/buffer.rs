use std::cmp;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::ops::Range;

use crate::canvas::Canvas;
use crate::coord::{Cursor, Pos, Size};
use crate::face::{Bg, Fg};
use crate::key::Key;
use crate::row::{Row, TAB_WIDTH};
use crate::syntax::{Indent, Syntax};

enum Draw {
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
    faces: Vec<(Fg, Bg)>,
}

pub struct Buffer {
    syntax: Box<dyn Syntax>,
    pub filename: Option<String>,
    pub modified: bool,
    pub pos: Pos,
    pub size: Size,
    offset: Pos,
    cursor: Cursor,
    anchor: Option<Pos>,
    hl_from: Option<usize>,
    draw: Draw,
    rows: Vec<Row>,
    search: Search,
}

impl Buffer {
    pub fn new(filename: Option<String>) -> io::Result<Self> {
        let mut buffer = Self {
            syntax: Syntax::detect(filename.as_deref()),
            filename,
            modified: false,
            pos: Pos::new(0, 0),
            size: Size::new(0, 0),
            offset: Pos::new(0, 0),
            cursor: Cursor::new(0, 0),
            anchor: None,
            hl_from: Some(0),
            draw: Draw::Whole,
            rows: Vec::new(),
            search: Default::default(),
        };
        buffer.init()?;
        Ok(buffer)
    }

    fn init(&mut self) -> io::Result<()> {
        if let Some(filename) = self.filename.as_deref() {
            let file = File::open(filename)?;
            let mut reader = BufReader::new(file);
            let mut buf = String::new();

            let crlf: &[_] = &['\r', '\n'];
            let mut ends_with_lf = false;

            while reader.read_line(&mut buf)? > 0 {
                let string = buf.trim_end_matches(crlf).to_string();
                self.rows.push(Row::new(string));
                ends_with_lf = buf.ends_with('\n');
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
        if let Some(filename) = self.filename.as_deref() {
            let file = File::create(filename)?;
            let mut writer = BufWriter::new(file);
            let len = self.rows.len();

            for (i, row) in self.rows.iter_mut().enumerate() {
                writer.write(row.string.as_bytes())?;
                if i < len - 1 {
                    writer.write(b"\n")?;
                }
                row.hl_context = 0;
            }
            self.syntax = Syntax::detect(Some(filename));
            self.modified = false;
            self.hl_from = Some(0);
            self.draw = Draw::Whole;
        }
        Ok(())
    }

    pub fn draw(&mut self, canvas: &mut Canvas) -> io::Result<()> {
        let (start, end) = (self.offset.y, self.offset.y + self.size.h);

        if let Some(y) = self.hl_from {
            let len = self.syntax.highlight(&mut self.rows[y..]);
            let y_range = match self.draw {
                Draw::None => start..start,
                Draw::Min => start.max(y)..end.min(y + len),
                Draw::End => start.max(y)..end,
                Draw::Whole => start..end,
            };
            self.draw_rows(canvas, y_range)?;
        } else if let Draw::Whole = self.draw {
            self.draw_rows(canvas, start..end)?;
        }

        self.draw_status_bar(canvas)?;

        self.hl_from = None;
        self.draw = Draw::None;
        Ok(())
    }

    fn draw_rows(&self, canvas: &mut Canvas, y_range: Range<usize>) -> io::Result<()> {
        write!(
            canvas,
            "\x1b[{};{}H",
            self.pos.y + y_range.start - self.offset.y + 1,
            self.pos.x + 1,
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

    fn draw_status_bar(&self, canvas: &mut Canvas) -> io::Result<()> {
        let filename = self.filename.as_deref().unwrap_or("newfile");
        let modified = if self.modified { "+" } else { "" };
        let cursor = format!("{}, {}", self.cursor.y + 1, self.cursor.x + 1);
        let syntax = self.syntax.name();

        let left_len = filename.len() + modified.len() + 2;
        let right_len = cursor.len() + syntax.len() + 4;
        let padding = self.size.w.saturating_sub(left_len + right_len);

        write!(
            canvas,
            "\x1b[{};{}H",
            self.pos.y + self.size.h + 1,
            self.pos.x + 1,
        )?;
        canvas.set_fg_color(Fg::Default)?;
        canvas.set_bg_color(Bg::StatusBar)?;

        if left_len <= self.size.w {
            canvas.write(b" ")?;
            canvas.write(filename.as_bytes())?;
            canvas.write(b" ")?;
            canvas.write(modified.as_bytes())?;
        }

        for _ in 0..padding {
            canvas.write(b" ")?;
        }

        if left_len + right_len <= self.size.w {
            canvas.write(b" ")?;
            canvas.write(cursor.as_bytes())?;
            canvas.write(b" ")?;
            canvas.write(self.syntax.color(canvas.term))?;
            canvas.write(b" ")?;
            canvas.write(syntax.as_bytes())?;
            canvas.write(b" ")?;
        }

        canvas.reset_color()?;
        Ok(())
    }

    pub fn draw_cursor(&self, canvas: &mut Canvas) -> io::Result<()> {
        write!(
            canvas,
            "\x1b[{};{}H",
            self.pos.y + self.cursor.y - self.offset.y + 1,
            self.pos.x + self.cursor.x - self.offset.x + 1,
        )?;
        Ok(())
    }

    pub fn process_keypress(&mut self, key: Key) {
        let prev_cursor = self.cursor;

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
            }
            Key::ArrowUp | Key::Ctrl(b'P') => {
                if self.cursor.y > 0 {
                    self.cursor.y -= 1;
                    self.cursor.x = self.rows[self.cursor.y].prev_fit_x(self.cursor.last_x);
                }
            }
            Key::ArrowDown | Key::Ctrl(b'N') => {
                if self.cursor.y < self.rows.len() - 1 {
                    self.cursor.y += 1;
                    self.cursor.x = self.rows[self.cursor.y].prev_fit_x(self.cursor.last_x);
                }
            }
            Key::Home | Key::Ctrl(b'A') => {
                let x = self.rows[self.cursor.y].first_letter_x();
                self.cursor.x = if self.cursor.x == x { 0 } else { x };
                self.cursor.last_x = self.cursor.x;
            }
            Key::End | Key::Ctrl(b'E') => {
                self.cursor.x = self.rows[self.cursor.y].max_x();
                self.cursor.last_x = self.cursor.x;
            }
            Key::PageUp | Key::Alt(b'v') => {
                if self.offset.y > 0 {
                    self.cursor.y -= cmp::min(self.size.h, self.offset.y);
                    self.offset.y -= cmp::min(self.size.h, self.offset.y);
                    self.cursor.x = self.rows[self.cursor.y].prev_fit_x(self.cursor.last_x);
                    self.draw = Draw::Whole;
                }
            }
            Key::PageDown | Key::Ctrl(b'V') => {
                if self.offset.y + self.size.h < self.rows.len() {
                    self.cursor.y += cmp::min(self.size.h, self.rows.len() - self.cursor.y - 1);
                    self.offset.y += self.size.h;
                    self.cursor.x = self.rows[self.cursor.y].prev_fit_x(self.cursor.last_x);
                    self.draw = Draw::Whole;
                }
            }
            Key::Backspace | Key::Ctrl(b'H') => {
                if self.cursor.x > 0 {
                    self.cursor.x = self.rows[self.cursor.y].prev_x(self.cursor.x);
                    self.cursor.last_x = self.cursor.x;
                    self.rows[self.cursor.y].remove(self.cursor.x);
                    self.modified = true;
                    self.hl_from = Some(self.cursor.y);
                    self.draw = Draw::Min;
                } else if self.cursor.y > 0 {
                    let row = self.rows.remove(self.cursor.y);
                    self.cursor.y -= 1;
                    self.cursor.x = self.rows[self.cursor.y].max_x();
                    self.cursor.last_x = self.cursor.x;
                    self.rows[self.cursor.y].push_str(&row.string);
                    self.modified = true;
                    self.hl_from = Some(self.cursor.y);
                    self.draw = Draw::End;
                }
            }
            Key::Delete | Key::Ctrl(b'D') => {
                if self.cursor.x < self.rows[self.cursor.y].max_x() {
                    self.rows[self.cursor.y].remove(self.cursor.x);
                    self.modified = true;
                    self.hl_from = Some(self.cursor.y);
                    self.draw = Draw::Min;
                } else if self.cursor.y < self.rows.len() - 1 {
                    let row = self.rows.remove(self.cursor.y + 1);
                    self.rows[self.cursor.y].push_str(&row.string);
                    self.modified = true;
                    self.hl_from = Some(self.cursor.y);
                    self.draw = Draw::End;
                }
            }
            Key::Ctrl(b'@') => {
                if let Some(anchor) = self.anchor {
                    self.unhighlight_region(anchor);
                }
                self.anchor = Some(self.cursor.as_pos());
            }
            Key::Ctrl(b'G') => {
                if let Some(anchor) = self.anchor {
                    self.unhighlight_region(anchor);
                }
                self.anchor = None;
                self.draw = Draw::Whole;
            }
            Key::Ctrl(b'I') => {
                match self.syntax.indent() {
                    Indent::None => {
                        self.rows[self.cursor.y].insert(self.cursor.x, '\t');
                        self.cursor.x = self.rows[self.cursor.y].next_x(self.cursor.x);
                    }
                    Indent::Tab => {
                        self.rows[self.cursor.y].insert(0, '\t');
                        self.cursor.x += TAB_WIDTH;
                    }
                    Indent::Spaces(n) => {
                        let x = self.rows[self.cursor.y].first_letter_x();
                        let spaces = " ".repeat(n - x % n);
                        self.rows[self.cursor.y].insert_str(0, &spaces);
                        self.cursor.x += spaces.len();
                    }
                }
                self.cursor.last_x = self.cursor.x;
                self.modified = true;
                self.hl_from = Some(self.cursor.y);
                self.draw = Draw::Min;
            }
            Key::Ctrl(b'J') | Key::Ctrl(b'M') => {
                let string = self.rows[self.cursor.y].split_off(self.cursor.x);
                self.rows.insert(self.cursor.y + 1, Row::new(string));
                self.cursor.y += 1;
                self.cursor.x = 0;
                self.cursor.last_x = self.cursor.x;
                self.modified = true;
                self.hl_from = Some(self.cursor.y - 1);
                self.draw = Draw::End;
            }
            Key::Ctrl(b'K') => {
                self.rows[self.cursor.y].truncate(self.cursor.x);
                self.modified = true;
                self.hl_from = Some(self.cursor.y);
                self.draw = Draw::Min;
            }
            Key::Ctrl(b'U') => {
                self.rows[self.cursor.y].remove_str(0, self.cursor.x);
                self.cursor.x = 0;
                self.cursor.last_x = self.cursor.x;
                self.modified = true;
                self.hl_from = Some(self.cursor.y);
                self.draw = Draw::Min;
            }
            Key::Alt(b'<') => {
                self.cursor.y = 0;
                self.cursor.x = 0;
                self.cursor.last_x = self.cursor.x;
            }
            Key::Alt(b'>') => {
                self.cursor.y = self.rows.len() - 1;
                self.cursor.x = self.rows[self.cursor.y].max_x();
                self.cursor.last_x = self.cursor.x;
            }
            Key::Char(ch) => {
                self.rows[self.cursor.y].insert(self.cursor.x, ch);
                self.cursor.x = self.rows[self.cursor.y].next_x(self.cursor.x);
                self.cursor.last_x = self.cursor.x;
                self.modified = true;
                self.hl_from = Some(self.cursor.y);
                self.draw = Draw::Min;
            }
            _ => (),
        }

        self.scroll();

        if let Some(_) = self.anchor {
            self.highlight_region(prev_cursor);
            self.draw = Draw::Whole; // temporary
        }
    }

    fn scroll(&mut self) {
        if self.cursor.y < self.offset.y {
            self.offset.y = self.cursor.y;
            self.draw = Draw::Whole;
        }
        if self.cursor.y >= self.offset.y + self.size.h {
            self.offset.y = self.cursor.y - self.size.h + 1;
            self.draw = Draw::Whole;
        }
        if self.cursor.x < self.offset.x {
            self.offset.x = self.cursor.x;
            self.draw = Draw::Whole;
        }
        if self.cursor.x >= self.offset.x + self.size.w {
            self.offset.x = self.cursor.x - self.size.w + 1;
            self.draw = Draw::Whole;
        }
    }

    fn highlight_region(&mut self, prev_cursor: Cursor) {
        let pos1 = cmp::min(prev_cursor.as_pos(), self.cursor.as_pos());
        let pos2 = cmp::max(prev_cursor.as_pos(), self.cursor.as_pos());

        for y in pos1.y..=pos2.y {
            let row = &mut self.rows[y];
            let x1 = if y == pos1.y { pos1.x } else { 0 };
            let x2 = if y == pos2.y { pos2.x } else { row.max_x() };

            for i in row.x_to_idx(x1)..row.x_to_idx(x2) {
                row.faces[i].1 = match row.faces[i].1 {
                    Bg::Default => Bg::Region,
                    _ => Bg::Default,
                };
            }
            if y < pos2.y {
                row.trailing_bg = match row.trailing_bg {
                    Bg::Default => Bg::Region,
                    _ => Bg::Default,
                };
            }
        }
    }

    fn unhighlight_region(&mut self, anchor: Pos) {
        let pos1 = cmp::min(anchor, self.cursor.as_pos());
        let pos2 = cmp::max(anchor, self.cursor.as_pos());

        for y in pos1.y..=pos2.y {
            let row = &mut self.rows[y];
            let x1 = if y == pos1.y { pos1.x } else { 0 };
            let x2 = if y == pos2.y { pos2.x } else { row.max_x() };

            for i in row.x_to_idx(x1)..row.x_to_idx(x2) {
                row.faces[i].1 = Bg::Default;
            }
            row.trailing_bg = Bg::Default;
        }
    }
}

impl Buffer {
    pub fn search(&mut self, query: &str, backward: bool) {
        for (y, row) in self.rows.iter_mut().enumerate() {
            for (idx, _) in row.string.match_indices(query) {
                let pos = Pos::new(row.idx_to_x(idx), y);
                let mut faces = vec![(Fg::Match, Bg::Match); query.len()];
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
        self.highlight_match(true);
        self.draw = Draw::Whole;
    }

    #[allow(clippy::collapsible_if)]
    pub fn next_match(&mut self, backward: bool) {
        if self.search.matches.len() <= 1 {
            return;
        }

        self.highlight_match(false);

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
        self.highlight_match(true);
        self.draw = Draw::Whole;
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
        self.draw = Draw::Whole;
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

    fn highlight_match(&mut self, current: bool) {
        let mat = &self.search.matches[self.search.match_idx];
        let row = &mut self.rows[mat.pos.y];
        let idx = row.x_to_idx(mat.pos.x);
        let face = if current {
            (Fg::CurrentMatch, Bg::CurrentMatch)
        } else {
            (Fg::Match, Bg::Match)
        };

        for i in idx..(idx + mat.faces.len()) {
            row.faces[i] = face;
        }
    }
}
