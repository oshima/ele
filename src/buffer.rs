use std::cmp;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::ops::Range;

use crate::canvas::Canvas;
use crate::coord::{Cursor, Pos, Size};
use crate::event::Event;
use crate::face::{Bg, Fg};
use crate::key::Key;
use crate::row::{str_width, Row, TAB_WIDTH};
use crate::syntax::{Indent, Syntax};
use crate::util::ExpandableRange;

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
    pos: Pos,
    size: Size,
    offset: Pos,
    cursor: Cursor,
    anchor: Option<Pos>,
    rows: Vec<Row>,
    draw_range: ExpandableRange,
    undo_list: Vec<Event>,
    redo_list: Vec<Event>,
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
            rows: Vec::new(),
            draw_range: Default::default(),
            undo_list: Vec::new(),
            redo_list: Vec::new(),
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
        self.highlight(0);
        self.draw_range.full_expand();
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
            self.anchor = None;
            self.modified = false;
            self.highlight(0);
        }
        Ok(())
    }

    pub fn resize(&mut self, pos: Pos, size: Size) {
        self.pos = pos;
        self.size = size;
        self.scroll();
        self.draw_range.full_expand();
    }

    pub fn draw(&mut self, canvas: &mut Canvas) -> io::Result<()> {
        if let (Some(start), Some(end)) = self.draw_range.as_tuple() {
            let (top, bottom) = (self.offset.y, self.offset.y + self.size.h);
            let y_range = start.max(top)..end.min(bottom);
            self.draw_rows(canvas, y_range)?;
        }
        self.draw_status_bar(canvas)?;

        self.draw_range.clear();
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
        canvas.write(b"\x1b[K")?;

        if left_len <= self.size.w {
            canvas.write(b" ")?;
            canvas.write(filename.as_bytes())?;
            canvas.write(b" ")?;
            canvas.write(modified.as_bytes())?;
            canvas.write(b"\x1b[K")?;
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
            canvas.reset_color()?;
            canvas.write(b"\x1b[K")?;
        }
        Ok(())
    }

    pub fn draw_cursor(&self, canvas: &mut Canvas) -> io::Result<()> {
        write!(
            canvas,
            "\x1b[{};{}H",
            self.pos.y + self.cursor.y - self.offset.y + 1,
            self.pos.x + self.cursor.x - self.offset.x + 1,
        )
    }

    pub fn process_keypress(&mut self, key: Key, clipboard: &mut String) {
        match key {
            Key::ArrowLeft | Key::Ctrl(b'B') => {
                if self.cursor.x > 0 {
                    let prev_cursor = self.cursor;
                    self.cursor.x = self.rows[self.cursor.y].prev_x(self.cursor.x);
                    self.cursor.last_x = self.cursor.x;
                    if self.anchor.is_some() {
                        self.highlight_region(prev_cursor);
                    }
                    self.scroll();
                } else if self.cursor.y > 0 {
                    let prev_cursor = self.cursor;
                    self.cursor.y -= 1;
                    self.cursor.x = self.rows[self.cursor.y].max_x();
                    self.cursor.last_x = self.cursor.x;
                    if self.anchor.is_some() {
                        self.highlight_region(prev_cursor);
                    }
                    self.scroll();
                }
            }
            Key::ArrowRight | Key::Ctrl(b'F') => {
                if self.cursor.x < self.rows[self.cursor.y].max_x() {
                    let prev_cursor = self.cursor;
                    self.cursor.x = self.rows[self.cursor.y].next_x(self.cursor.x);
                    self.cursor.last_x = self.cursor.x;
                    if self.anchor.is_some() {
                        self.highlight_region(prev_cursor);
                    }
                    self.scroll();
                } else if self.cursor.y < self.rows.len() - 1 {
                    let prev_cursor = self.cursor;
                    self.cursor.y += 1;
                    self.cursor.x = 0;
                    self.cursor.last_x = self.cursor.x;
                    if self.anchor.is_some() {
                        self.highlight_region(prev_cursor);
                    }
                    self.scroll();
                }
            }
            Key::ArrowUp | Key::Ctrl(b'P') => {
                if self.cursor.y > 0 {
                    let prev_cursor = self.cursor;
                    self.cursor.y -= 1;
                    self.cursor.x = self.rows[self.cursor.y].prev_fit_x(self.cursor.last_x);
                    if self.anchor.is_some() {
                        self.highlight_region(prev_cursor);
                    }
                    self.scroll();
                }
            }
            Key::ArrowDown | Key::Ctrl(b'N') => {
                if self.cursor.y < self.rows.len() - 1 {
                    let prev_cursor = self.cursor;
                    self.cursor.y += 1;
                    self.cursor.x = self.rows[self.cursor.y].prev_fit_x(self.cursor.last_x);
                    if self.anchor.is_some() {
                        self.highlight_region(prev_cursor);
                    }
                    self.scroll();
                }
            }
            Key::Home | Key::Ctrl(b'A') => {
                let prev_cursor = self.cursor;
                let x = self.rows[self.cursor.y].first_letter_x();
                self.cursor.x = if self.cursor.x == x { 0 } else { x };
                self.cursor.last_x = self.cursor.x;
                if self.anchor.is_some() {
                    self.highlight_region(prev_cursor);
                }
                self.scroll();
            }
            Key::End | Key::Ctrl(b'E') => {
                let prev_cursor = self.cursor;
                self.cursor.x = self.rows[self.cursor.y].max_x();
                self.cursor.last_x = self.cursor.x;
                if self.anchor.is_some() {
                    self.highlight_region(prev_cursor);
                }
                self.scroll();
            }
            Key::PageUp | Key::Alt(b'v') => {
                if self.offset.y > 0 {
                    let prev_cursor = self.cursor;
                    self.cursor.y -= cmp::min(self.size.h, self.offset.y);
                    self.offset.y -= cmp::min(self.size.h, self.offset.y);
                    self.cursor.x = self.rows[self.cursor.y].prev_fit_x(self.cursor.last_x);
                    if self.anchor.is_some() {
                        self.highlight_region(prev_cursor);
                    }
                    self.scroll();
                    self.draw_range.full_expand();
                }
            }
            Key::PageDown | Key::Ctrl(b'V') => {
                if self.offset.y + self.size.h < self.rows.len() {
                    let prev_cursor = self.cursor;
                    self.cursor.y += cmp::min(self.size.h, self.rows.len() - self.cursor.y - 1);
                    self.offset.y += self.size.h;
                    self.cursor.x = self.rows[self.cursor.y].prev_fit_x(self.cursor.last_x);
                    if self.anchor.is_some() {
                        self.highlight_region(prev_cursor);
                    }
                    self.scroll();
                    self.draw_range.full_expand();
                }
            }
            Key::Backspace | Key::Ctrl(b'H') => {
                if let Some(anchor) = self.anchor {
                    let prev_cursor = self.cursor;
                    let string = self.remove_region(anchor);
                    let moved = self.cursor.as_pos() != prev_cursor.as_pos();
                    self.undo_list
                        .push(Event::Delete(self.cursor.as_pos(), string, moved));
                    self.anchor = None;
                    self.modified = true;
                    self.highlight(self.cursor.y);
                    self.scroll();
                } else if self.cursor.x > 0 {
                    self.cursor.x = self.rows[self.cursor.y].prev_x(self.cursor.x);
                    self.cursor.last_x = self.cursor.x;
                    let string = self.rows[self.cursor.y].remove(self.cursor.x);
                    self.undo_list
                        .push(Event::Delete(self.cursor.as_pos(), string, true));
                    self.modified = true;
                    self.highlight(self.cursor.y);
                    self.scroll();
                } else if self.cursor.y > 0 {
                    let row = self.rows.remove(self.cursor.y);
                    self.cursor.y -= 1;
                    self.cursor.x = self.rows[self.cursor.y].max_x();
                    self.cursor.last_x = self.cursor.x;
                    self.rows[self.cursor.y].push_str(&row.string);
                    self.undo_list.push(Event::Delete(
                        self.cursor.as_pos(),
                        "\n".to_string(),
                        true,
                    ));
                    self.modified = true;
                    self.highlight(self.cursor.y);
                    self.scroll();
                    self.draw_range.full_expand_end();
                }
            }
            Key::Delete | Key::Ctrl(b'D') => {
                if let Some(anchor) = self.anchor {
                    let prev_cursor = self.cursor;
                    let string = self.remove_region(anchor);
                    let moved = self.cursor.as_pos() != prev_cursor.as_pos();
                    self.undo_list
                        .push(Event::Delete(self.cursor.as_pos(), string, moved));
                    self.anchor = None;
                    self.modified = true;
                    self.highlight(self.cursor.y);
                    self.scroll();
                } else if self.cursor.x < self.rows[self.cursor.y].max_x() {
                    let string = self.rows[self.cursor.y].remove(self.cursor.x);
                    self.undo_list
                        .push(Event::Delete(self.cursor.as_pos(), string, false));
                    self.modified = true;
                    self.highlight(self.cursor.y);
                } else if self.cursor.y < self.rows.len() - 1 {
                    let row = self.rows.remove(self.cursor.y + 1);
                    self.rows[self.cursor.y].push_str(&row.string);
                    self.undo_list.push(Event::Delete(
                        self.cursor.as_pos(),
                        "\n".to_string(),
                        false,
                    ));
                    self.modified = true;
                    self.highlight(self.cursor.y);
                    self.draw_range.full_expand_end();
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
            }
            Key::Ctrl(b'I') => {
                if self.anchor.is_some() {
                    return; // TODO
                }
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
                self.highlight(self.cursor.y);
                self.scroll();
            }
            Key::Ctrl(b'J') | Key::Ctrl(b'M') => {
                if let Some(anchor) = self.anchor {
                    self.remove_region(anchor);
                    self.anchor = None;
                }
                self.undo_list
                    .push(Event::Insert(self.cursor.as_pos(), "\n".to_string(), true));
                let string = self.rows[self.cursor.y].split_off(self.cursor.x);
                self.rows.insert(self.cursor.y + 1, Row::new(string));
                self.cursor.y += 1;
                self.cursor.x = 0;
                self.cursor.last_x = self.cursor.x;
                self.modified = true;
                self.highlight(self.cursor.y - 1);
                self.scroll();
                self.draw_range.full_expand_end();
            }
            Key::Ctrl(b'K') => {
                if let Some(anchor) = self.anchor {
                    self.unhighlight_region(anchor);
                    self.anchor = None;
                }
                let string = self.rows[self.cursor.y].split_off(self.cursor.x);
                clipboard.clear();
                clipboard.push_str(&string);
                self.undo_list
                    .push(Event::Delete(self.cursor.as_pos(), string, false));
                self.modified = true;
                self.highlight(self.cursor.y);
            }
            Key::Ctrl(b'U') => {
                if let Some(anchor) = self.anchor {
                    self.unhighlight_region(anchor);
                    self.anchor = None;
                }
                let string = self.rows[self.cursor.y].remove_str(0, self.cursor.x);
                self.cursor.x = 0;
                self.cursor.last_x = self.cursor.x;
                clipboard.clear();
                clipboard.push_str(&string);
                self.undo_list
                    .push(Event::Delete(self.cursor.as_pos(), string, true));
                self.modified = true;
                self.highlight(self.cursor.y);
                self.scroll();
            }
            Key::Ctrl(b'W') => {
                if let Some(anchor) = self.anchor {
                    let prev_cursor = self.cursor;
                    let string = self.remove_region(anchor);
                    let moved = self.cursor.as_pos() != prev_cursor.as_pos();
                    clipboard.clear();
                    clipboard.push_str(&string);
                    self.undo_list
                        .push(Event::Delete(self.cursor.as_pos(), string, moved));
                    self.anchor = None;
                    self.modified = true;
                    self.highlight(self.cursor.y);
                    self.scroll();
                }
            }
            Key::Ctrl(b'Y') => {
                self.undo_list
                    .push(Event::Insert(self.cursor.as_pos(), clipboard.clone(), true));
                let lines: Vec<&str> = clipboard.split('\n').collect();
                if lines.len() == 1 {
                    let max_x = self.rows[self.cursor.y].max_x();
                    self.rows[self.cursor.y].insert_str(self.cursor.x, &lines[0]);
                    self.cursor.x += self.rows[self.cursor.y].max_x() - max_x;
                    self.cursor.last_x = self.cursor.x;
                    self.modified = true;
                    self.highlight(self.cursor.y);
                    self.scroll();
                } else if lines.len() > 1 {
                    let string = self.rows[self.cursor.y].split_off(self.cursor.x);
                    let mut rows = self.rows.split_off(self.cursor.y + 1);
                    self.rows[self.cursor.y].push_str(&lines[0]);
                    self.rows
                        .append(&mut lines[1..].iter().map(|s| Row::new(s.to_string())).collect());
                    self.cursor.y += lines.len() - 1;
                    self.cursor.x = self.rows[self.cursor.y].max_x();
                    self.cursor.last_x = self.cursor.x;
                    self.rows[self.cursor.y].push_str(&string);
                    self.rows.append(&mut rows);
                    self.modified = true;
                    self.highlight(self.cursor.y - lines.len() + 1);
                    self.scroll();
                    self.draw_range.full_expand_end();
                }
            }
            Key::Ctrl(b'_') => {
                if let Some(event) = self.undo_list.pop() {
                    self.process_event(event.reverse());
                }
            }
            Key::Alt(b'<') => {
                let prev_cursor = self.cursor;
                self.cursor.y = 0;
                self.cursor.x = 0;
                self.cursor.last_x = self.cursor.x;
                if self.anchor.is_some() {
                    self.highlight_region(prev_cursor);
                }
                self.scroll();
            }
            Key::Alt(b'>') => {
                let prev_cursor = self.cursor;
                self.cursor.y = self.rows.len() - 1;
                self.cursor.x = self.rows[self.cursor.y].max_x();
                self.cursor.last_x = self.cursor.x;
                if self.anchor.is_some() {
                    self.highlight_region(prev_cursor);
                }
                self.scroll();
            }
            Key::Alt(b'w') => {
                if let Some(anchor) = self.anchor {
                    let string = self.extract_region(anchor);
                    clipboard.clear();
                    clipboard.push_str(&string);
                    self.unhighlight_region(anchor);
                    self.anchor = None;
                }
            }
            Key::Char(ch) => {
                if let Some(anchor) = self.anchor {
                    self.remove_region(anchor);
                    self.anchor = None;
                }
                self.undo_list
                    .push(Event::Insert(self.cursor.as_pos(), ch.to_string(), true));
                let max_x = self.rows[self.cursor.y].max_x();
                self.rows[self.cursor.y].insert(self.cursor.x, ch);
                self.cursor.x += self.rows[self.cursor.y].max_x() - max_x;
                self.cursor.last_x = self.cursor.x;
                self.modified = true;
                self.highlight(self.cursor.y);
                self.scroll();
            }
            _ => (),
        }
    }

    fn process_event(&mut self, event: Event) {
        match event {
            Event::Insert(pos, string, mv) => {
                let lines: Vec<&str> = string.split('\n').collect();
                if lines.len() == 1 {
                    let pos2 = Pos::new(pos.x + str_width(pos.x, lines[0]), pos.y);
                    self.rows[pos.y].insert_str(pos.x, lines[0]);
                    self.highlight(pos.y);
                    self.move_to_pos(if mv { pos2 } else { pos });
                } else if lines.len() > 1 {
                    let pos2 = Pos::new(
                        str_width(0, lines[lines.len() - 1]),
                        pos.y + lines.len() - 1,
                    );
                    let string = self.rows[pos.y].split_off(pos.x);
                    let mut rows = self.rows.split_off(pos.y + 1);
                    self.rows[pos.y].push_str(&lines[0]);
                    self.rows
                        .append(&mut lines[1..].iter().map(|s| Row::new(s.to_string())).collect());
                    self.rows[pos2.y].push_str(&string);
                    self.rows.append(&mut rows);
                    self.highlight(pos.y);
                    self.move_to_pos(if mv { pos2 } else { pos });
                    self.draw_range.full_expand_end();
                }
            }
            Event::Delete(pos, string, _) => {
                let lines: Vec<&str> = string.split('\n').collect();
                if lines.len() == 1 {
                    let pos2 = Pos::new(pos.x + str_width(pos.x, lines[0]), pos.y);
                    self.rows[pos.y].remove_str(pos.x, pos2.x);
                    self.highlight(pos.y);
                    self.move_to_pos(pos);
                } else {
                    let pos2 = Pos::new(
                        str_width(0, lines[lines.len() - 1]),
                        pos.y + lines.len() - 1,
                    );
                    let string = self.rows[pos2.y].split_off(pos2.x);
                    let mut rows = self.rows.split_off(pos2.y + 1);
                    self.rows[pos.y].truncate(pos.x);
                    self.rows[pos.y].push_str(&string);
                    self.rows.truncate(pos.y + 1);
                    self.rows.append(&mut rows);
                    self.highlight(pos.y);
                    self.move_to_pos(pos);
                    self.draw_range.full_expand_end();
                }
            }
        }
    }

    fn highlight(&mut self, y: usize) {
        let len = self.syntax.highlight(&mut self.rows[y..]);
        self.draw_range.expand(y, y + len);
    }

    fn scroll(&mut self) {
        if self.cursor.y < self.offset.y {
            self.offset.y = self.cursor.y;
            self.draw_range.full_expand();
        }
        if self.cursor.y >= self.offset.y + self.size.h {
            self.offset.y = self.cursor.y - self.size.h + 1;
            self.draw_range.full_expand();
        }
        if self.cursor.x < self.offset.x {
            self.offset.x = self.cursor.x;
            self.draw_range.full_expand();
        }
        if self.cursor.x >= self.offset.x + self.size.w {
            self.offset.x = self.cursor.x - self.size.w + 1;
            self.draw_range.full_expand();
        }
    }

    fn move_to_pos(&mut self, pos: Pos) {
        if pos.x < self.offset.x || pos.x >= self.offset.x + self.size.w {
            self.offset.x = pos.x.saturating_sub(self.size.w / 2);
            self.draw_range.full_expand();
        }
        if pos.y < self.offset.y || pos.y >= self.offset.y + self.size.h {
            self.offset.y = pos.y.saturating_sub(self.size.h / 2);
            self.draw_range.full_expand();
        }
        self.cursor.x = pos.x;
        self.cursor.y = pos.y;
        self.cursor.last_x = pos.x;
    }
}

impl Buffer {
    fn extract_region(&self, anchor: Pos) -> String {
        let pos1 = cmp::min(anchor, self.cursor.as_pos());
        let pos2 = cmp::max(anchor, self.cursor.as_pos());
        let mut region = Vec::new();

        for y in pos1.y..=pos2.y {
            let row = &self.rows[y];
            let x1 = if y == pos1.y { pos1.x } else { 0 };
            let x2 = if y == pos2.y { pos2.x } else { row.max_x() };
            let s = &row.string[row.x_to_idx(x1)..row.x_to_idx(x2)];
            region.push(s.to_string());
        }
        region.join("\n")
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
        self.draw_range.expand(pos1.y, pos2.y + 1);
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
            if y < pos2.y {
                row.trailing_bg = Bg::Default;
            }
        }
        self.draw_range.expand(pos1.y, pos2.y + 1);
    }

    fn remove_region(&mut self, anchor: Pos) -> String {
        let pos1 = cmp::min(anchor, self.cursor.as_pos());
        let pos2 = cmp::max(anchor, self.cursor.as_pos());

        self.cursor.y = pos1.y;
        self.cursor.x = pos1.x;
        self.cursor.last_x = self.cursor.x;

        if pos1.y == pos2.y {
            let region = self.rows[pos1.y].remove_str(pos1.x, pos2.x);
            self.draw_range.expand(pos1.y, pos1.y + 1);
            region
        } else {
            let mut region = vec![self.rows[pos1.y].split_off(pos1.x)];
            let string = self.rows[pos2.y].split_off(pos2.x);
            let mut rows = self.rows.split_off(pos2.y + 1);
            self.rows[pos1.y].push_str(&string);
            self.rows[pos1.y].trailing_bg = Bg::Default;
            region.append(
                &mut self
                    .rows
                    .split_off(pos1.y + 1)
                    .into_iter()
                    .map(|row| row.string)
                    .collect(),
            );
            self.rows.append(&mut rows);
            self.draw_range.expand_start(pos1.y);
            self.draw_range.full_expand_end();
            region.join("\n")
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
        self.draw_range.full_expand();
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
        self.draw_range.full_expand();
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
        } else if self.anchor.is_some() {
            self.highlight_region(self.search.orig_cursor);
        }
        self.draw_range.full_expand();
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
