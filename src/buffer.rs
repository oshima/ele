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
use crate::syntax::Syntax;
use crate::util::DrawRange;

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
    draw_range: DrawRange,
    undo: bool,
    undo_list: Vec<Event>,
    redo_list: Vec<Event>,
    next_eid: usize,
    saved_eid: Option<usize>,
    last_key: Option<Key>,
    search: Search,
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

impl Buffer {
    pub fn new(filename: Option<String>) -> io::Result<Self> {
        let mut buffer = Self {
            syntax: <dyn Syntax>::detect(filename.as_deref()),
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
            next_eid: 0,
            saved_eid: None,
            last_key: None,
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
        self.syntax_update(0);
        self.draw_range.full_expand();
        Ok(())
    }

    pub fn resize(&mut self, pos: Pos, size: Size) {
        self.pos = pos;
        self.size = size;
        self.scroll();
        self.draw_range.full_expand();
    }

    pub fn draw(&mut self, canvas: &mut Canvas) -> io::Result<()> {
        if let Some((start, end)) = self.draw_range.as_tuple() {
            let y_range = start.max(self.offset.y)..end.min(self.offset.y + self.size.h);
            let x_range = self.offset.x..(self.offset.x + self.size.w);

            canvas.set_cursor(self.pos.x, self.pos.y + y_range.start - self.offset.y)?;
            self.rows.draw(canvas, x_range, y_range)?;

            self.draw_range.clear();
        }

        canvas.set_cursor(self.pos.x, self.pos.y + self.size.h)?;
        self.draw_status_bar(canvas)
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

        canvas.write_repeat(b" ", padding)?;

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
        canvas.set_cursor(
            self.pos.x + self.cursor.x - self.offset.x,
            self.pos.y + self.cursor.y - self.offset.y,
        )
    }

    #[allow(clippy::collapsible_else_if)]
    pub fn process_key(&mut self, key: Key, clipboard: &mut String) -> &str {
        let message = match key {
            Key::ArrowLeft | Key::Ctrl(b'B') => {
                if let Some(pos) = self.rows.prev_pos(self.cursor) {
                    if self.anchor.is_some() {
                        self.highlight_region(pos);
                    }
                    self.cursor = pos;
                    self.saved_x = pos.x;
                    self.scroll();
                }
                ""
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
                ""
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
                ""
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
                ""
            }
            Key::Home | Key::Ctrl(b'A') => {
                let x = self.rows[self.cursor.y].indent_width();
                let pos = Pos::new(if self.cursor.x == x { 0 } else { x }, self.cursor.y);
                if self.anchor.is_some() {
                    self.highlight_region(pos);
                }
                self.cursor = pos;
                self.saved_x = pos.x;
                self.scroll();
                ""
            }
            Key::End | Key::Ctrl(b'E') => {
                let pos = Pos::new(self.rows[self.cursor.y].last_x(), self.cursor.y);
                if self.anchor.is_some() {
                    self.highlight_region(pos);
                }
                self.cursor = pos;
                self.saved_x = pos.x;
                self.scroll();
                ""
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
                ""
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
                ""
            }
            Key::Backspace | Key::Ctrl(b'H') => {
                if let Some(anchor) = self.anchor {
                    self.remove_region(anchor);
                    self.anchor = None;
                } else if let Some(pos) = self.rows.prev_pos(self.cursor) {
                    let event = Event::Remove(self.eid(), pos, self.cursor, true);
                    let event = self.process_event(event);
                    if let Some(Key::Backspace | Key::Ctrl(b'H')) = self.last_key {
                        self.merge_event(event);
                    } else {
                        self.push_event(event);
                    }
                    self.scroll();
                }
                ""
            }
            Key::Delete | Key::Ctrl(b'D') => {
                if let Some(anchor) = self.anchor {
                    self.remove_region(anchor);
                    self.anchor = None;
                } else if let Some(pos) = self.rows.next_pos(self.cursor) {
                    let event = Event::Remove(self.eid(), self.cursor, pos, false);
                    let event = self.process_event(event);
                    if let Some(Key::Delete | Key::Ctrl(b'D')) = self.last_key {
                        self.merge_event(event);
                    } else {
                        self.push_event(event);
                    }
                }
                ""
            }
            Key::Ctrl(b'@') => {
                if let Some(anchor) = self.anchor {
                    self.unhighlight_region(anchor);
                }
                self.anchor = Some(self.cursor);
                "Mark set"
            }
            Key::Ctrl(b'G') => {
                if let Some(anchor) = self.anchor {
                    self.unhighlight_region(anchor);
                }
                self.anchor = None;
                "Quit"
            }
            Key::Ctrl(b'I') => {
                if let Some(unit) = self.syntax.indent_unit() {
                    if let Some(anchor) = self.anchor {
                        self.unhighlight_region(anchor);
                        self.indent_region(anchor, unit);
                        self.anchor = None;
                    } else {
                        let string = unit.repeat(self.rows[self.cursor.y].indent_level);
                        if self.rows[self.cursor.y].indent_part() != string {
                            let event = Event::Indent(self.eid(), self.cursor, string);
                            let event = self.process_event(event);
                            self.push_event(event);
                        } else {
                            let x = self.rows[self.cursor.y].indent_width();
                            if self.cursor.x < x {
                                self.cursor.x = x;
                                self.saved_x = x;
                            }
                        }
                    }
                } else {
                    if let Some(anchor) = self.anchor {
                        self.remove_region(anchor);
                        self.anchor = None;
                    }
                    let event = Event::Insert(self.eid(), self.cursor, "\t".into(), true);
                    let event = self.process_event(event);
                    if let Some(Key::Ctrl(b'I')) = self.last_key {
                        self.merge_event(event);
                    } else {
                        self.push_event(event);
                    }
                }
                self.scroll();
                ""
            }
            Key::Ctrl(b'J' | b'M') => {
                if let Some(anchor) = self.anchor {
                    self.remove_region(anchor);
                    self.anchor = None;
                }

                let eid = if let Some(Key::Ctrl(b'J' | b'M')) = self.last_key {
                    self.undo_list.last().unwrap().id()
                } else {
                    self.eid()
                };

                let event = Event::Insert(eid, self.cursor, "\n".into(), true);
                let cursor1 = self.cursor;
                let event = self.process_event(event);
                let cursor2 = self.cursor;
                self.push_event(event);

                self.cursor = cursor1;
                if self.rows[self.cursor.y].is_whitespace() {
                    if !self.rows[self.cursor.y].is_empty() {
                        let event = Event::Indent(eid, self.cursor, "".into());
                        let event = self.process_event(event);
                        self.push_event(event);
                    }
                } else if let Some(unit) = self.syntax.indent_unit() {
                    let string = unit.repeat(self.rows[self.cursor.y].indent_level);
                    if self.rows[self.cursor.y].indent_part() != string {
                        let event = Event::Indent(eid, self.cursor, string);
                        let event = self.process_event(event);
                        self.push_event(event);
                    }
                }

                self.cursor = cursor2;
                if let Some(unit) = self.syntax.indent_unit() {
                    let string = unit.repeat(self.rows[self.cursor.y].indent_level);
                    if self.rows[self.cursor.y].indent_part() != string {
                        let event = Event::Indent(eid, self.cursor, string);
                        let event = self.process_event(event);
                        self.push_event(event);
                    }
                }
                self.scroll();
                ""
            }
            Key::Ctrl(b'K') => {
                if let Some(anchor) = self.anchor {
                    self.unhighlight_region(anchor);
                    self.anchor = None;
                }
                let pos = Pos::new(self.rows[self.cursor.y].last_x(), self.cursor.y);
                clipboard.clear();
                clipboard.push_str(&self.rows.read_str(self.cursor, pos));
                let event = Event::Remove(self.eid(), self.cursor, pos, false);
                let event = self.process_event(event);
                self.push_event(event);
                ""
            }
            Key::Ctrl(b'U') => {
                if let Some(anchor) = self.anchor {
                    self.unhighlight_region(anchor);
                    self.anchor = None;
                }
                let pos = Pos::new(0, self.cursor.y);
                clipboard.clear();
                clipboard.push_str(&self.rows.read_str(pos, self.cursor));
                let event = Event::Remove(self.eid(), pos, self.cursor, true);
                let event = self.process_event(event);
                self.push_event(event);
                self.scroll();
                ""
            }
            Key::Ctrl(b'W') => {
                if let Some(anchor) = self.anchor {
                    clipboard.clear();
                    clipboard.push_str(&self.read_region(anchor));
                    self.remove_region(anchor);
                    self.anchor = None;
                }
                ""
            }
            Key::Ctrl(b'Y') => {
                if let Some(anchor) = self.anchor {
                    self.remove_region(anchor);
                    self.anchor = None;
                }
                let event = Event::Insert(self.eid(), self.cursor, clipboard.clone(), true);
                let event = self.process_event(event);
                self.push_event(event);
                self.scroll();
                ""
            }
            Key::Ctrl(b'_') => {
                if let Some(anchor) = self.anchor {
                    self.unhighlight_region(anchor);
                    self.anchor = None;
                }
                if !matches!(self.last_key, Some(Key::Ctrl(b'_'))) {
                    self.undo = !self.undo;
                }
                if self.undo {
                    if let Some(eid) = self.undo_list.last().map(|e| e.id()) {
                        while self.undo_list.last().map_or(false, |e| e.id() == eid) {
                            let event = self.undo_list.pop().unwrap();
                            let event = self.process_event(event);
                            self.redo_list.push(event);
                        }
                        self.scroll_center();
                        "Undo"
                    } else {
                        "No further undo information"
                    }
                } else {
                    if let Some(eid) = self.redo_list.last().map(|e| e.id()) {
                        while self.redo_list.last().map_or(false, |e| e.id() == eid) {
                            let event = self.redo_list.pop().unwrap();
                            let event = self.process_event(event);
                            self.undo_list.push(event);
                        }
                        self.scroll_center();
                        "Redo"
                    } else {
                        "No further redo information"
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
                ""
            }
            Key::Alt(b'>') => {
                let pos = self.rows.last_pos();
                if self.anchor.is_some() {
                    self.highlight_region(pos);
                }
                self.cursor = pos;
                self.saved_x = pos.x;
                self.scroll();
                ""
            }
            Key::Alt(b'b') => {
                if let Some(pos) = self.rows.prev_word_pos(self.cursor) {
                    if self.anchor.is_some() {
                        self.highlight_region(pos);
                    }
                    self.cursor = pos;
                    self.saved_x = pos.x;
                    self.scroll();
                }
                ""
            }
            Key::Alt(b'd') => {
                if let Some(anchor) = self.anchor {
                    self.unhighlight_region(anchor);
                    self.anchor = None;
                }
                if let Some(pos) = self.rows.next_word_pos(self.cursor) {
                    let event = Event::Remove(self.eid(), self.cursor, pos, false);
                    let event = self.process_event(event);
                    self.push_event(event);
                }
                ""
            }
            Key::Alt(b'f') => {
                if let Some(pos) = self.rows.next_word_pos(self.cursor) {
                    if self.anchor.is_some() {
                        self.highlight_region(pos);
                    }
                    self.cursor = pos;
                    self.saved_x = pos.x;
                    self.scroll();
                }
                ""
            }
            Key::Alt(b'h') => {
                if let Some(anchor) = self.anchor {
                    self.unhighlight_region(anchor);
                    self.anchor = None;
                }
                if let Some(pos) = self.rows.prev_word_pos(self.cursor) {
                    let event = Event::Remove(self.eid(), pos, self.cursor, true);
                    let event = self.process_event(event);
                    self.push_event(event);
                    self.scroll();
                }
                ""
            }
            Key::Alt(b'w') => {
                if let Some(anchor) = self.anchor {
                    clipboard.clear();
                    clipboard.push_str(&self.read_region(anchor));
                    self.unhighlight_region(anchor);
                    self.anchor = None;
                }
                ""
            }
            Key::Char(ch) => {
                if let Some(anchor) = self.anchor {
                    self.remove_region(anchor);
                    self.anchor = None;
                }
                let event = Event::Insert(self.eid(), self.cursor, ch.into(), true);
                let event = self.process_event(event);
                if let Some(Key::Char(_)) = self.last_key {
                    self.merge_event(event);
                } else {
                    self.push_event(event);
                }
                self.scroll();
                ""
            }
            _ => "",
        };

        self.last_key = Some(key);

        message
    }

    fn syntax_update(&mut self, y: usize) {
        let len = self.syntax.update_rows(&mut self.rows[y..]);
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
    pub fn modified(&self) -> bool {
        self.saved_eid != self.undo_list.last().map(|e| e.id())
    }

    fn eid(&mut self) -> usize {
        let eid = self.next_eid;
        self.next_eid += 1;
        eid
    }

    fn process_event(&mut self, event: Event) -> Event {
        match event {
            Event::Insert(id, pos1, string, mv) => {
                let pos2 = self.rows.insert_str(pos1, &string);
                self.cursor = if mv { pos2 } else { pos1 };
                self.saved_x = (if mv { pos2 } else { pos1 }).x;
                self.syntax_update(pos1.y);
                if pos1.y < pos2.y {
                    self.draw_range.full_expand_end();
                }
                Event::Remove(id, pos1, pos2, mv)
            }
            Event::Remove(id, pos1, pos2, mv) => {
                let string = self.rows.remove_str(pos1, pos2);
                self.cursor = pos1;
                self.saved_x = pos1.x;
                self.syntax_update(pos1.y);
                if pos1.y < pos2.y {
                    self.draw_range.full_expand_end();
                }
                Event::Insert(id, pos1, string, mv)
            }
            Event::Indent(id, pos, string) => {
                let width1 = self.rows[pos.y].indent_width();
                let string = self.rows[pos.y].indent(&string);
                let width2 = self.rows[pos.y].indent_width();
                let x = if width1 < width2 {
                    pos.x.saturating_add(width2 - width1).max(width2)
                } else {
                    pos.x.saturating_sub(width1 - width2).max(width2)
                };
                let pos = Pos::new(x, pos.y);
                self.cursor = pos;
                self.saved_x = pos.x;
                self.syntax_update(pos.y);
                Event::Indent(id, pos, string)
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
        let event = last_event.merge(event);
        self.undo_list.push(event);
    }
}

impl Buffer {
    fn read_region(&self, anchor: Pos) -> String {
        let pos1 = self.cursor.min(anchor);
        let pos2 = self.cursor.max(anchor);
        self.rows.read_str(pos1, pos2)
    }

    fn highlight_region(&mut self, pos: Pos) {
        let pos1 = self.cursor.min(pos);
        let pos2 = self.cursor.max(pos);

        for y in pos1.y..=pos2.y {
            let row = &mut self.rows[y];
            let x1 = if y == pos1.y { pos1.x } else { 0 };
            let x2 = if y == pos2.y { pos2.x } else { row.last_x() };

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
            let x2 = if y == pos2.y { pos2.x } else { row.last_x() };

            for i in row.x_to_idx(x1)..row.x_to_idx(x2) {
                row.faces[i].1 = Bg::Default;
            }
            if y < pos2.y {
                row.trailing_bg = Bg::Default;
            }
        }
        self.draw_range.expand(pos1.y, pos2.y + 1);
    }

    fn indent_region(&mut self, anchor: Pos, unit: &str) {
        let pos1 = self.cursor.min(anchor);
        let pos2 = self.cursor.max(anchor);
        let eid = self.eid();

        for y in pos1.y..=pos2.y {
            let string = unit.repeat(self.rows[y].indent_level);
            if self.rows[y].is_whitespace() {
                if !self.rows[y].is_empty() {
                    let event = Event::Indent(eid, Pos::new(0, y), "".into());
                    let event = self.process_event(event);
                    self.push_event(event);
                }
            } else if self.rows[y].indent_part() != string {
                let event = Event::Indent(eid, Pos::new(0, y), string);
                let event = self.process_event(event);
                self.push_event(event);
            }
        }
    }

    fn remove_region(&mut self, anchor: Pos) {
        let pos1 = self.cursor.min(anchor);
        let pos2 = self.cursor.max(anchor);
        let event = Event::Remove(self.eid(), pos1, pos2, self.cursor > anchor);
        let event = self.process_event(event);
        self.push_event(event);
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

    #[allow(clippy::collapsible_else_if)]
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

impl Buffer {
    pub fn go_to_line(&mut self, num: usize) {
        let y = num.saturating_sub(1);
        let y = y.min(self.rows.last_pos().y);
        self.cursor = Pos::new(0, y);
        self.saved_x = 0;
        self.scroll_center();
    }
}

impl Buffer {
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
                row.context = None;
            }

            self.syntax = <dyn Syntax>::detect(Some(filename));
            self.anchor = None;
            self.last_key = None;
            self.syntax_update(0);

            self.saved_eid = self.undo_list.last().map(|e| e.id());
        }
        Ok(())
    }
}
