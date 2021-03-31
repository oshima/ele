use std::cmp;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};

use crate::canvas::Canvas;
use crate::coord::{Pos, Size};
use crate::event::Event;
use crate::face::{Bg, Fg};
use crate::key::Key;
use crate::row::Row;
use crate::rows::{Rows, RowsMethods};
use crate::syntax::{Indent, Syntax};
use crate::util::ExpandableRange;

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
    pos: Pos,
    size: Size,
    offset: Pos,
    cursor: Pos,
    anchor: Option<Pos>,
    saved_x: usize,
    rows: Rows,
    draw_range: ExpandableRange,
    undo: bool,
    undo_list: Vec<Event>,
    redo_list: Vec<Event>,
    last_key: Option<Key>,
    saved_at: Option<u64>,
    search: Search,
}

impl Buffer {
    pub fn new(filename: Option<String>) -> io::Result<Self> {
        let mut buffer = Self {
            syntax: Syntax::detect(filename.as_deref()),
            filename,
            pos: Pos::new(0, 0),
            size: Size::new(0, 0),
            offset: Pos::new(0, 0),
            cursor: Pos::new(0, 0),
            anchor: None,
            saved_x: 0,
            rows: Rows::new(),
            draw_range: Default::default(),
            undo: false,
            undo_list: Vec::new(),
            redo_list: Vec::new(),
            last_key: None,
            saved_at: None,
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

    pub fn save(&mut self, clock: u64) -> io::Result<()> {
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
            self.last_key = None;
            self.highlight(0);

            if let Some(event) = self.undo_list.last_mut() {
                event.stamp(clock);
                self.saved_at = Some(clock);
            } else {
                self.saved_at = None;
            }
        }
        Ok(())
    }

    pub fn modified(&self) -> bool {
        if let Some(saved_at) = self.saved_at {
            if let Some(event) = self.undo_list.last() {
                event.get_stamp() != Some(saved_at)
            } else {
                true
            }
        } else {
            !self.undo_list.is_empty()
        }
    }

    pub fn resize(&mut self, pos: Pos, size: Size) {
        self.pos = pos;
        self.size = size;
        self.scroll();
        self.draw_range.full_expand();
    }

    pub fn draw(&mut self, canvas: &mut Canvas) -> io::Result<()> {
        if let Some((start, end)) = self.draw_range.as_tuple() {
            let (top, bottom) = (self.offset.y, self.offset.y + self.size.h);
            let y_range = start.max(top)..end.min(bottom);
            let x_range = self.offset.x..(self.offset.x + self.size.w);
            write!(
                canvas,
                "\x1b[{};{}H",
                self.pos.y + y_range.start - self.offset.y + 1,
                self.pos.x + 1,
            )?;
            self.rows.draw(canvas, x_range, y_range)?;
        }

        write!(
            canvas,
            "\x1b[{};{}H",
            self.pos.y + self.size.h + 1,
            self.pos.x + 1,
        )?;
        self.draw_status_bar(canvas)?;

        self.draw_range.clear();
        Ok(())
    }

    fn draw_status_bar(&self, canvas: &mut Canvas) -> io::Result<()> {
        let filename = self.filename.as_deref().unwrap_or("newfile");
        let modified = if self.modified() { "+" } else { "" };
        let cursor = format!("{}, {}", self.cursor.y + 1, self.cursor.x + 1);
        let syntax = self.syntax.name();

        let left_len = filename.len() + modified.len() + 2;
        let right_len = cursor.len() + syntax.len() + 4;
        let padding = self.size.w.saturating_sub(left_len + right_len);

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
            canvas.write(self.syntax.fg_color(canvas.term))?;
            canvas.write(self.syntax.bg_color(canvas.term))?;
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
            }
            Key::ArrowUp | Key::Ctrl(b'P') => {
                if self.cursor.y > 0 {
                    let pos = Pos::new(
                        self.rows[self.cursor.y - 1].prev_fit_x(self.saved_x),
                        self.cursor.y - 1,
                    );
                    if self.anchor.is_some() {
                        self.highlight_region(pos);
                    }
                    self.cursor = pos;
                    self.scroll();
                }
            }
            Key::ArrowDown | Key::Ctrl(b'N') => {
                if self.cursor.y < self.rows.len() - 1 {
                    let pos = Pos::new(
                        self.rows[self.cursor.y + 1].prev_fit_x(self.saved_x),
                        self.cursor.y + 1,
                    );
                    if self.anchor.is_some() {
                        self.highlight_region(pos);
                    }
                    self.cursor = pos;
                    self.scroll();
                }
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
            }
            Key::End | Key::Ctrl(b'E') => {
                let pos = Pos::new(self.rows[self.cursor.y].max_x(), self.cursor.y);
                if self.anchor.is_some() {
                    self.highlight_region(pos);
                }
                self.cursor = pos;
                self.saved_x = pos.x;
                self.scroll();
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
            }
            Key::Backspace | Key::Ctrl(b'H') => {
                if let Some(anchor) = self.anchor {
                    self.remove_region(anchor);
                    self.anchor = None;
                } else if let Some(pos) = self.rows.prev_pos(self.cursor) {
                    let event = Event::RemoveMv(pos, self.cursor, None);
                    let revent = self.process_event(event);
                    if let Some(Key::Backspace) | Some(Key::Ctrl(b'H')) = self.last_key {
                        self.merge_event(revent);
                    } else {
                        self.push_event(revent);
                    }
                    self.scroll();
                }
            }
            Key::Delete | Key::Ctrl(b'D') => {
                if let Some(anchor) = self.anchor {
                    self.remove_region(anchor);
                    self.anchor = None;
                } else if let Some(pos) = self.rows.next_pos(self.cursor) {
                    let event = Event::Remove(self.cursor, pos, None);
                    let revent = self.process_event(event);
                    if let Some(Key::Delete) | Some(Key::Ctrl(b'D')) = self.last_key {
                        self.merge_event(revent);
                    } else {
                        self.push_event(revent);
                    }
                }
            }
            Key::Ctrl(b'@') => {
                if let Some(anchor) = self.anchor {
                    self.unhighlight_region(anchor);
                }
                self.anchor = Some(self.cursor);
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
                let event = match self.syntax.indent() {
                    Indent::None => Event::InsertMv(self.cursor, "\t".into(), None),
                    Indent::Tab => Event::Indent(self.cursor, "\t".into(), None),
                    Indent::Spaces(n) => {
                        let x = self.rows[self.cursor.y].first_letter_x();
                        Event::Indent(self.cursor, " ".repeat(n - x % n), None)
                    }
                };
                let revent = self.process_event(event);
                if let Some(Key::Ctrl(b'I')) = self.last_key {
                    self.merge_event(revent);
                } else {
                    self.push_event(revent);
                }
                self.scroll();
            }
            Key::Ctrl(b'J') | Key::Ctrl(b'M') => {
                if let Some(anchor) = self.anchor {
                    self.remove_region(anchor);
                    self.anchor = None;
                }
                let event = Event::InsertMv(self.cursor, "\n".into(), None);
                let revent = self.process_event(event);
                if let Some(Key::Ctrl(b'J')) | Some(Key::Ctrl(b'M')) = self.last_key {
                    self.merge_event(revent);
                } else {
                    self.push_event(revent);
                }
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
                let event = Event::Remove(self.cursor, pos, None);
                let revent = self.process_event(event);
                self.push_event(revent);
            }
            Key::Ctrl(b'U') => {
                if let Some(anchor) = self.anchor {
                    self.unhighlight_region(anchor);
                    self.anchor = None;
                }
                let pos = Pos::new(0, self.cursor.y);
                clipboard.clear();
                clipboard.push_str(&self.rows.read_text(pos, self.cursor));
                let event = Event::RemoveMv(pos, self.cursor, None);
                let revent = self.process_event(event);
                self.push_event(revent);
                self.scroll();
            }
            Key::Ctrl(b'W') => {
                if let Some(anchor) = self.anchor {
                    clipboard.clear();
                    clipboard.push_str(&self.read_region(anchor));
                    self.remove_region(anchor);
                    self.anchor = None;
                }
            }
            Key::Ctrl(b'Y') => {
                if let Some(anchor) = self.anchor {
                    self.remove_region(anchor);
                    self.anchor = None;
                }
                let event = Event::InsertMv(self.cursor, clipboard.clone(), None);
                let revent = self.process_event(event);
                self.push_event(revent);
                self.scroll();
            }
            Key::Ctrl(b'_') => {
                if !matches!(self.last_key, Some(Key::Ctrl(b'_'))) {
                    self.undo = !self.undo;
                }
                if self.undo {
                    if let Some(event) = self.undo_list.pop() {
                        let revent = self.process_event(event);
                        self.redo_list.push(revent);
                        self.scroll_center();
                    }
                } else {
                    if let Some(event) = self.redo_list.pop() {
                        let revent = self.process_event(event);
                        self.undo_list.push(revent);
                        self.scroll_center();
                    }
                }
            }
            Key::Alt(b'<') => {
                let pos = Pos::new(0, 0);
                if self.anchor.is_some() {
                    self.highlight_region(pos);
                }
                self.cursor = pos;
                self.saved_x = pos.x;
                self.scroll();
            }
            Key::Alt(b'>') => {
                let pos = self.rows.max_pos();
                if self.anchor.is_some() {
                    self.highlight_region(pos);
                }
                self.cursor = pos;
                self.saved_x = pos.x;
                self.scroll();
            }
            Key::Alt(b'w') => {
                if let Some(anchor) = self.anchor {
                    clipboard.clear();
                    clipboard.push_str(&self.read_region(anchor));
                    self.unhighlight_region(anchor);
                    self.anchor = None;
                }
            }
            Key::Char(ch) => {
                if let Some(anchor) = self.anchor {
                    self.remove_region(anchor);
                    self.anchor = None;
                }
                let event = Event::InsertMv(self.cursor, ch.to_string(), None);
                let revent = self.process_event(event);
                if let Some(Key::Char(_)) = self.last_key {
                    self.merge_event(revent);
                } else {
                    self.push_event(revent);
                }
                self.scroll();
            }
            _ => (),
        }

        self.last_key = Some(key);
    }

    fn process_event(&mut self, event: Event) -> Event {
        match event {
            Event::Insert(pos1, string, stamp) => {
                let pos2 = self.rows.insert_text(pos1, &string);
                self.cursor = pos1;
                self.saved_x = pos1.x;
                self.highlight(pos1.y);
                if pos1.y < pos2.y {
                    self.draw_range.full_expand_end();
                }
                Event::Remove(pos1, pos2, stamp)
            }
            Event::InsertMv(pos1, string, stamp) => {
                let pos2 = self.rows.insert_text(pos1, &string);
                self.cursor = pos2;
                self.saved_x = pos2.x;
                self.highlight(pos1.y);
                if pos1.y < pos2.y {
                    self.draw_range.full_expand_end();
                }
                Event::RemoveMv(pos1, pos2, stamp)
            }
            Event::Remove(pos1, pos2, stamp) => {
                let string = self.rows.remove_text(pos1, pos2);
                self.cursor = pos1;
                self.saved_x = pos1.x;
                self.highlight(pos1.y);
                if pos1.y < pos2.y {
                    self.draw_range.full_expand_end();
                }
                Event::Insert(pos1, string, stamp)
            }
            Event::RemoveMv(pos1, pos2, stamp) => {
                let string = self.rows.remove_text(pos1, pos2);
                self.cursor = pos1;
                self.saved_x = pos1.x;
                self.highlight(pos1.y);
                if pos1.y < pos2.y {
                    self.draw_range.full_expand_end();
                }
                Event::InsertMv(pos1, string, stamp)
            }
            Event::Indent(pos, string, stamp) => {
                let width = self.rows[pos.y].insert_str(0, &string);
                self.cursor = Pos::new(pos.x + width, pos.y);
                self.saved_x = pos.x + width;
                self.highlight(pos.y);
                Event::Unindent(self.cursor, width, stamp)
            }
            Event::Unindent(pos, width, stamp) => {
                let string = self.rows[pos.y].remove_str(0, width);
                self.cursor = Pos::new(pos.x - width, pos.y);
                self.saved_x = pos.x - width;
                self.highlight(pos.y);
                Event::Indent(self.cursor, string, stamp)
            }
        }
    }

    fn push_event(&mut self, event: Event) {
        self.undo_list.push(event);
        self.redo_list.clear();
        self.undo = false;
    }

    fn merge_event(&mut self, event: Event) {
        let last_event = self.undo_list.pop().unwrap();
        let event = last_event.merge(event).unwrap();
        self.undo_list.push(event);
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
        let event = if self.cursor < anchor {
            Event::Remove(pos1, pos2, None)
        } else {
            Event::RemoveMv(pos1, pos2, None)
        };
        let revent = self.process_event(event);
        self.push_event(revent);
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
