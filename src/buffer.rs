use std::cmp;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::ops::Range;

use self::UndoRedo::*;
use crate::canvas::Canvas;
use crate::coord::{Pos, Size};
use crate::event::Event;
use crate::face::{Bg, Fg};
use crate::key::Key;
use crate::row::Row;
use crate::rows::{Rows, RowsMethods};
use crate::syntax::{Indent, Syntax};
use crate::util::ExpandableRange;

#[derive(Clone, Copy)]
enum UndoRedo {
    WillUndo,
    WillRedo,
    Undoing,
    Redoing,
}

#[derive(Default)]
struct Search {
    matches: Vec<Match>,
    match_idx: usize,
    orig_offset: Pos,
    orig_cursor: Pos,
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
    cursor: Pos,
    anchor: Option<Pos>,
    saved_x: usize,
    rows: Rows,
    draw_range: ExpandableRange,
    undo_list: Vec<Event>,
    redo_list: Vec<Event>,
    undo_redo: UndoRedo,
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
            cursor: Pos::new(0, 0),
            anchor: None,
            saved_x: 0,
            rows: Rows::new(),
            draw_range: Default::default(),
            undo_list: Vec::new(),
            redo_list: Vec::new(),
            undo_redo: WillUndo,
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
                if let Some(pos) = self.rows.prev_pos(self.cursor) {
                    if self.anchor.is_some() {
                        self.highlight_region(pos);
                    }
                    self.cursor = pos;
                    self.saved_x = pos.x;
                    self.scroll();
                }
                self.switch_undo_redo();
            }
            Key::ArrowRight | Key::Ctrl(b'F') => {
                if let Some(pos) = self.rows.next_pos(self.cursor) {
                    if self.anchor.is_some() {
                        self.highlight_region(pos);
                    }
                    self.cursor = pos;
                    self.saved_x = pos.x;
                    self.scroll();
                }
                self.switch_undo_redo();
            }
            Key::ArrowUp | Key::Ctrl(b'P') => {
                if let Some(pos) = self.rows.upper_pos(Pos::new(self.saved_x, self.cursor.y)) {
                    if self.anchor.is_some() {
                        self.highlight_region(pos);
                    }
                    self.cursor = pos;
                    self.scroll();
                }
                self.switch_undo_redo();
            }
            Key::ArrowDown | Key::Ctrl(b'N') => {
                if let Some(pos) = self.rows.lower_pos(Pos::new(self.saved_x, self.cursor.y)) {
                    if self.anchor.is_some() {
                        self.highlight_region(pos);
                    }
                    self.cursor = pos;
                    self.scroll();
                }
                self.switch_undo_redo();
            }
            Key::Home | Key::Ctrl(b'A') => {
                let x = self.rows[self.cursor.y].first_letter_x();
                let pos = Pos::new(if self.cursor.x == x { 0 } else { x }, self.cursor.y);
                if self.anchor.is_some() {
                    self.highlight_region(pos);
                }
                self.cursor = pos;
                self.saved_x = pos.x;
                self.scroll();
                self.switch_undo_redo();
            }
            Key::End | Key::Ctrl(b'E') => {
                let pos = Pos::new(self.rows[self.cursor.y].max_x(), self.cursor.y);
                if self.anchor.is_some() {
                    self.highlight_region(pos);
                }
                self.cursor = pos;
                self.saved_x = pos.x;
                self.scroll();
                self.switch_undo_redo();
            }
            Key::PageUp | Key::Alt(b'v') => {
                if self.offset.y > 0 {
                    let delta = cmp::min(self.size.h, self.offset.y);
                    let pos = Pos::new(
                        self.rows[self.cursor.y - delta].prev_fit_x(self.saved_x),
                        self.cursor.y - delta,
                    );
                    if self.anchor.is_some() {
                        self.highlight_region(pos);
                    }
                    self.cursor = pos;
                    self.offset.y -= delta;
                    self.draw_range.full_expand();
                }
                self.switch_undo_redo();
            }
            Key::PageDown | Key::Ctrl(b'V') => {
                if self.offset.y + self.size.h < self.rows.len() {
                    let delta = cmp::min(self.size.h, self.rows.len() - 1 - self.cursor.y);
                    let pos = Pos::new(
                        self.rows[self.cursor.y + delta].prev_fit_x(self.saved_x),
                        self.cursor.y + delta,
                    );
                    if self.anchor.is_some() {
                        self.highlight_region(pos);
                    }
                    self.cursor = pos;
                    self.offset.y += self.size.h;
                    self.draw_range.full_expand();
                }
                self.switch_undo_redo();
            }
            Key::Backspace | Key::Ctrl(b'H') => {
                if let Some(anchor) = self.anchor {
                    self.remove_region(anchor);
                    self.anchor = None;
                } else if let Some(pos) = self.rows.prev_pos(self.cursor) {
                    let event = Event::Delete(pos, self.cursor, true);
                    let reverse = self.process_event(event);
                    self.undo_list.push(reverse);
                    self.redo_list.clear();
                    self.undo_redo = WillUndo;
                    self.scroll();
                }
            }
            Key::Delete | Key::Ctrl(b'D') => {
                if let Some(anchor) = self.anchor {
                    self.remove_region(anchor);
                    self.anchor = None;
                } else if let Some(pos) = self.rows.next_pos(self.cursor) {
                    let event = Event::Delete(self.cursor, pos, false);
                    let reverse = self.process_event(event);
                    self.undo_list.push(reverse);
                    self.redo_list.clear();
                    self.undo_redo = WillUndo;
                }
            }
            Key::Ctrl(b'@') => {
                if let Some(anchor) = self.anchor {
                    self.unhighlight_region(anchor);
                }
                self.anchor = Some(self.cursor);
                self.switch_undo_redo();
            }
            Key::Ctrl(b'G') => {
                if let Some(anchor) = self.anchor {
                    self.unhighlight_region(anchor);
                }
                self.anchor = None;
                self.switch_undo_redo();
            }
            Key::Ctrl(b'I') => {
                if self.anchor.is_some() {
                    return; // TODO
                }
                let event = match self.syntax.indent() {
                    Indent::None => Event::Insert(self.cursor, "\t".into(), true),
                    Indent::Tab => Event::Indent(self.cursor, "\t".into()),
                    Indent::Spaces(n) => {
                        let x = self.rows[self.cursor.y].first_letter_x();
                        Event::Indent(self.cursor, " ".repeat(n - x % n))
                    }
                };
                let reverse = self.process_event(event);
                self.undo_list.push(reverse);
                self.redo_list.clear();
                self.undo_redo = WillUndo;
                self.scroll();
            }
            Key::Ctrl(b'J') | Key::Ctrl(b'M') => {
                if let Some(anchor) = self.anchor {
                    self.remove_region(anchor);
                    self.anchor = None;
                }
                let event = Event::Insert(self.cursor, "\n".into(), true);
                let reverse = self.process_event(event);
                self.undo_list.push(reverse);
                self.redo_list.clear();
                self.undo_redo = WillUndo;
                self.scroll();
            }
            Key::Ctrl(b'K') => {
                if let Some(anchor) = self.anchor {
                    self.unhighlight_region(anchor);
                    self.anchor = None;
                }
                let pos = Pos::new(self.rows[self.cursor.y].max_x(), self.cursor.y);
                clipboard.clear();
                clipboard.push_str(&self.rows.read_text(self.cursor, pos));
                let event = Event::Delete(self.cursor, pos, false);
                let reverse = self.process_event(event);
                self.undo_list.push(reverse);
                self.redo_list.clear();
                self.undo_redo = WillUndo;
            }
            Key::Ctrl(b'U') => {
                if let Some(anchor) = self.anchor {
                    self.unhighlight_region(anchor);
                    self.anchor = None;
                }
                let pos = Pos::new(0, self.cursor.y);
                clipboard.clear();
                clipboard.push_str(&self.rows.read_text(pos, self.cursor));
                let event = Event::Delete(pos, self.cursor, true);
                let reverse = self.process_event(event);
                self.undo_list.push(reverse);
                self.redo_list.clear();
                self.undo_redo = WillUndo;
                self.scroll();
            }
            Key::Ctrl(b'W') => {
                if let Some(anchor) = self.anchor {
                    clipboard.clear();
                    clipboard.push_str(&self.read_region(anchor));
                    self.remove_region(anchor);
                    self.anchor = None;
                }
                self.switch_undo_redo();
            }
            Key::Ctrl(b'Y') => {
                if let Some(anchor) = self.anchor {
                    self.remove_region(anchor);
                    self.anchor = None;
                }
                let event = Event::Insert(self.cursor, clipboard.clone(), true);
                let reverse = self.process_event(event);
                self.undo_list.push(reverse);
                self.redo_list.clear();
                self.undo_redo = WillUndo;
                self.scroll();
            }
            Key::Ctrl(b'_') => match self.undo_redo {
                WillUndo | Undoing => {
                    if let Some(event) = self.undo_list.pop() {
                        let reverse = self.process_event(event);
                        self.redo_list.push(reverse);
                        self.undo_redo = Undoing;
                        self.scroll_center();
                    }
                }
                WillRedo | Redoing => {
                    if let Some(event) = self.redo_list.pop() {
                        let reverse = self.process_event(event);
                        self.undo_list.push(reverse);
                        self.undo_redo = Redoing;
                        self.scroll_center();
                    }
                }
            },
            Key::Alt(b'<') => {
                let pos = Pos::new(0, 0);
                if self.anchor.is_some() {
                    self.highlight_region(pos);
                }
                self.cursor = pos;
                self.saved_x = pos.x;
                self.scroll();
                self.switch_undo_redo();
            }
            Key::Alt(b'>') => {
                let pos = self.rows.max_pos();
                if self.anchor.is_some() {
                    self.highlight_region(pos);
                }
                self.cursor = pos;
                self.saved_x = pos.x;
                self.scroll();
                self.switch_undo_redo();
            }
            Key::Alt(b'w') => {
                if let Some(anchor) = self.anchor {
                    clipboard.clear();
                    clipboard.push_str(&self.read_region(anchor));
                    self.unhighlight_region(anchor);
                    self.anchor = None;
                }
                self.switch_undo_redo();
            }
            Key::Char(ch) => {
                if let Some(anchor) = self.anchor {
                    self.remove_region(anchor);
                    self.anchor = None;
                }
                let event = Event::Insert(self.cursor, ch.to_string(), true);
                let reverse = self.process_event(event);
                self.undo_list.push(reverse);
                self.redo_list.clear();
                self.undo_redo = WillUndo;
                self.scroll();
            }
            _ => (),
        }
    }

    fn process_event(&mut self, event: Event) -> Event {
        match event {
            Event::Insert(pos1, string, mv) => {
                let pos2 = self.rows.insert_text(pos1, &string);
                self.cursor = if mv { pos2 } else { pos1 };
                self.saved_x = if mv { pos2.x } else { pos1.x };
                self.highlight(pos1.y);
                if pos1.y < pos2.y {
                    self.draw_range.full_expand_end();
                }
                Event::Delete(pos1, pos2, mv)
            }
            Event::Delete(pos1, pos2, mv) => {
                let string = self.rows.remove_text(pos1, pos2);
                self.cursor = pos1;
                self.saved_x = pos1.x;
                self.highlight(pos1.y);
                if pos1.y < pos2.y {
                    self.draw_range.full_expand_end();
                }
                Event::Insert(pos1, string, mv)
            }
            Event::Indent(pos, string) => {
                let width = self.rows[pos.y].insert_str(0, &string);
                self.cursor = Pos::new(pos.x + width, pos.y);
                self.saved_x = pos.x + width;
                self.highlight(pos.y);
                Event::Unindent(self.cursor, width)
            }
            Event::Unindent(pos, width) => {
                let string = self.rows[pos.y].remove_str(0, width);
                self.cursor = Pos::new(pos.x - width, pos.y);
                self.saved_x = pos.x - width;
                self.highlight(pos.y);
                Event::Indent(self.cursor, string)
            }
        }
    }

    fn switch_undo_redo(&mut self) {
        match self.undo_redo {
            Undoing => self.undo_redo = WillRedo,
            Redoing => self.undo_redo = WillUndo,
            _ => (),
        };
    }

    fn highlight(&mut self, y: usize) {
        let len = self.syntax.highlight(&mut self.rows[y..]);
        self.draw_range.expand(y, y + len);
    }

    fn scroll(&mut self) {
        if self.cursor.x < self.offset.x {
            self.offset.x = self.cursor.x;
            self.draw_range.full_expand();
        }
        if self.cursor.x >= self.offset.x + self.size.w {
            self.offset.x = self.cursor.x - self.size.w + 1;
            self.draw_range.full_expand();
        }
        if self.cursor.y < self.offset.y {
            self.offset.y = self.cursor.y;
            self.draw_range.full_expand();
        }
        if self.cursor.y >= self.offset.y + self.size.h {
            self.offset.y = self.cursor.y - self.size.h + 1;
            self.draw_range.full_expand();
        }
    }

    fn scroll_center(&mut self) {
        if self.cursor.x < self.offset.x || self.cursor.x >= self.offset.x + self.size.w {
            self.offset.x = self.cursor.x.saturating_sub(self.size.w / 2);
            self.draw_range.full_expand();
        }
        if self.cursor.y < self.offset.y || self.cursor.y >= self.offset.y + self.size.h {
            self.offset.y = self.cursor.y.saturating_sub(self.size.h / 2);
            self.draw_range.full_expand();
        }
    }
}

impl Buffer {
    fn read_region(&self, anchor: Pos) -> String {
        let pos1 = self.cursor.min(anchor);
        let pos2 = self.cursor.max(anchor);
        self.rows.read_text(pos1, pos2)
    }

    fn highlight_region(&mut self, pos: Pos) {
        let pos1 = self.cursor.min(pos);
        let pos2 = self.cursor.max(pos);

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
        let pos1 = self.cursor.min(anchor);
        let pos2 = self.cursor.max(anchor);

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

    fn remove_region(&mut self, anchor: Pos) {
        let pos1 = self.cursor.min(anchor);
        let pos2 = self.cursor.max(anchor);
        let mv = pos2 == self.cursor;
        let event = Event::Delete(pos1, pos2, mv);
        let reverse = self.process_event(event);
        self.undo_list.push(reverse);
        self.redo_list.clear();
        self.undo_redo = WillUndo;
        self.scroll();
        self.rows[pos1.y].trailing_bg = Bg::Default;
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
        self.search.match_idx = if backward {
            matches
                .rposition(|m| m.pos < self.cursor)
                .unwrap_or(self.search.matches.len() - 1)
        } else {
            matches.position(|m| m.pos >= self.cursor).unwrap_or(0)
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
        } else {
            self.saved_x = self.cursor.x;
            if self.anchor.is_some() {
                self.highlight_region(self.search.orig_cursor);
            }
        }
        self.draw_range.full_expand();
    }

    fn move_to_match(&mut self) {
        let mat = &self.search.matches[self.search.match_idx];
        self.cursor = mat.pos;
        self.scroll_center();
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
