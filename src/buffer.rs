use std::cmp;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};

use crate::canvas::Canvas;
use crate::coord::{Pos, Size};
use crate::edit::{Edit, EditKind};
use crate::face::{Bg, Fg};
use crate::key::Key;
use crate::row::Row;
use crate::rows::{Rows, RowsMethods};
use crate::syntax::Syntax;
use crate::util::DrawRange;

pub struct Buffer {
    pub file_path: Option<String>,
    syntax: Box<dyn Syntax>,
    pos: Pos,
    size: Size,
    offset: Pos,
    cursor: Pos,
    anchor: Option<Pos>,
    saved_x: usize,
    rows: Rows,
    draw_range: DrawRange,
    undo: bool,
    undo_list: Vec<Edit>,
    redo_list: Vec<Edit>,
    time: usize,
    saved_time: Option<usize>,
    last_key: Option<Key>,
    search: Search,
}

#[derive(Default)]
struct Search {
    matches: Vec<Match>,
    index: usize,
    orig_offset: Pos,
    orig_cursor: Pos,
}

struct Match {
    pos: Pos,
    faces: Vec<(Fg, Bg)>,
}

impl Buffer {
    pub fn new(file_path: Option<&str>) -> io::Result<Self> {
        let mut buffer = Self {
            file_path: file_path.map(|s| String::from(s)),
            syntax: <dyn Syntax>::detect(file_path),
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
            time: 0,
            saved_time: None,
            last_key: None,
            search: Default::default(),
        };
        buffer.init()?;
        Ok(buffer)
    }

    fn init(&mut self) -> io::Result<()> {
        if let Some(file_path) = self.file_path.as_deref() {
            let file = File::open(file_path)?;
            let mut reader = BufReader::new(file);
            let mut buf = String::new();

            let crlf: &[_] = &['\r', '\n'];
            let mut ends_with_lf = false;

            while reader.read_line(&mut buf)? > 0 {
                let string = buf.trim_end_matches(crlf);
                self.rows.push(Row::new(string));
                ends_with_lf = buf.ends_with('\n');
                buf.clear();
            }
            if self.rows.is_empty() || ends_with_lf {
                self.rows.push(Row::new(""));
            }
        } else {
            self.rows.push(Row::new(""));
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
        let file_path = self.file_path.as_deref().unwrap_or("newfile");
        let modified = if self.modified() { "+" } else { "" };
        let cursor = format!("{}, {}", self.cursor.y + 1, self.cursor.x + 1);
        let syntax = self.syntax.name();

        let left_len = file_path.len() + modified.len() + 2;
        let right_len = cursor.len() + syntax.len() + 4;
        let padding = self.size.w.saturating_sub(left_len + right_len);

        canvas.set_fg_color(Fg::Default)?;
        canvas.set_bg_color(Bg::StatusBar)?;
        canvas.write(b"\x1b[K")?;

        if left_len <= self.size.w {
            canvas.write(b" ")?;
            canvas.write(file_path.as_bytes())?;
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
        let mut save_key = true;

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
                    save_key = false;
                } else if let Some(pos) = self.rows.prev_pos(self.cursor) {
                    let edit = Edit::remove(self.time(), pos, self.cursor, true);
                    let edit = self.process_edit(edit);
                    if let Some(Key::Backspace | Key::Ctrl(b'H')) = self.last_key {
                        self.merge_edit(edit);
                    } else {
                        self.push_edit(edit);
                    }
                    self.scroll();
                }
                ""
            }
            Key::Delete | Key::Ctrl(b'D') => {
                if let Some(anchor) = self.anchor {
                    self.remove_region(anchor);
                    self.anchor = None;
                    save_key = false;
                } else if let Some(pos) = self.rows.next_pos(self.cursor) {
                    let edit = Edit::remove(self.time(), self.cursor, pos, false);
                    let edit = self.process_edit(edit);
                    if let Some(Key::Delete | Key::Ctrl(b'D')) = self.last_key {
                        self.merge_edit(edit);
                    } else {
                        self.push_edit(edit);
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
                            let edit = Edit::indent(self.time(), self.cursor, string);
                            let edit = self.process_edit(edit);
                            self.push_edit(edit);
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
                    let edit = Edit::insert(self.time(), self.cursor, "\t".into(), true);
                    let edit = self.process_edit(edit);
                    if let Some(Key::Ctrl(b'I')) = self.last_key {
                        self.merge_edit(edit);
                    } else {
                        self.push_edit(edit);
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

                let time = if let Some(Key::Ctrl(b'J' | b'M')) = self.last_key {
                    self.undo_list.last().unwrap().time
                } else {
                    self.time()
                };

                let edit = Edit::insert(time, self.cursor, "\n".into(), true);
                let cursor1 = self.cursor;
                let edit = self.process_edit(edit);
                let cursor2 = self.cursor;
                self.push_edit(edit);

                self.cursor = cursor1;
                if self.rows[self.cursor.y].is_whitespace() {
                    if !self.rows[self.cursor.y].is_empty() {
                        let edit = Edit::indent(time, self.cursor, "".into());
                        let edit = self.process_edit(edit);
                        self.push_edit(edit);
                    }
                } else if let Some(unit) = self.syntax.indent_unit() {
                    let string = unit.repeat(self.rows[self.cursor.y].indent_level);
                    if self.rows[self.cursor.y].indent_part() != string {
                        let edit = Edit::indent(time, self.cursor, string);
                        let edit = self.process_edit(edit);
                        self.push_edit(edit);
                    }
                }

                self.cursor = cursor2;
                if let Some(unit) = self.syntax.indent_unit() {
                    let string = unit.repeat(self.rows[self.cursor.y].indent_level);
                    if self.rows[self.cursor.y].indent_part() != string {
                        let edit = Edit::indent(time, self.cursor, string);
                        let edit = self.process_edit(edit);
                        self.push_edit(edit);
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
                let edit = Edit::remove(self.time(), self.cursor, pos, false);
                let edit = self.process_edit(edit);
                self.push_edit(edit);
                ""
            }
            Key::Ctrl(b'L') => {
                self.offset.y = if let Some(Key::Ctrl(b'L')) = self.last_key {
                    if self.offset.y == self.cursor.y.saturating_sub(self.size.h / 2) {
                        self.cursor.y
                    } else if self.offset.y == self.cursor.y {
                        self.cursor.y.saturating_sub(self.size.h - 1)
                    } else {
                        self.cursor.y.saturating_sub(self.size.h / 2)
                    }
                } else {
                    self.cursor.y.saturating_sub(self.size.h / 2)
                };
                self.draw_range.full_expand();
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
                let edit = Edit::remove(self.time(), pos, self.cursor, true);
                let edit = self.process_edit(edit);
                self.push_edit(edit);
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
                let edit = Edit::insert(self.time(), self.cursor, clipboard.clone(), true);
                let edit = self.process_edit(edit);
                self.push_edit(edit);
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
                    if let Some(time) = self.undo_list.last().map(|e| e.time) {
                        while self.undo_list.last().map_or(false, |e| e.time == time) {
                            let edit = self.undo_list.pop().unwrap();
                            let edit = self.process_edit(edit);
                            self.redo_list.push(edit);
                        }
                        self.scroll_center();
                        "Undo"
                    } else {
                        "No further undo information"
                    }
                } else {
                    if let Some(time) = self.redo_list.last().map(|e| e.time) {
                        while self.redo_list.last().map_or(false, |e| e.time == time) {
                            let edit = self.redo_list.pop().unwrap();
                            let edit = self.process_edit(edit);
                            self.undo_list.push(edit);
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
                    let edit = Edit::remove(self.time(), self.cursor, pos, false);
                    let edit = self.process_edit(edit);
                    self.push_edit(edit);
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
                    let edit = Edit::remove(self.time(), pos, self.cursor, true);
                    let edit = self.process_edit(edit);
                    self.push_edit(edit);
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
                let edit = Edit::insert(self.time(), self.cursor, ch.into(), true);
                let edit = self.process_edit(edit);
                if let Some(Key::Char(_)) = self.last_key {
                    self.merge_edit(edit);
                } else {
                    self.push_edit(edit);
                }
                self.scroll();
                ""
            }
            _ => "",
        };

        self.last_key = save_key.then(|| key);

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
        self.saved_time != self.undo_list.last().map(|e| e.time)
    }

    fn time(&mut self) -> usize {
        let time = self.time;
        self.time += 1;
        time
    }

    fn process_edit(&mut self, edit: Edit) -> Edit {
        let kind = match edit.kind {
            EditKind::Insert(pos1, string, mv) => {
                let pos2 = self.rows.insert_str(pos1, &string);
                self.cursor = if mv { pos2 } else { pos1 };
                self.saved_x = (if mv { pos2 } else { pos1 }).x;
                self.syntax_update(pos1.y);
                if pos1.y < pos2.y {
                    self.draw_range.full_expand_end();
                }
                EditKind::Remove(pos1, pos2, mv)
            }
            EditKind::Remove(pos1, pos2, mv) => {
                let string = self.rows.remove_str(pos1, pos2);
                self.cursor = pos1;
                self.saved_x = pos1.x;
                self.syntax_update(pos1.y);
                if pos1.y < pos2.y {
                    self.draw_range.full_expand_end();
                }
                EditKind::Insert(pos1, string, mv)
            }
            EditKind::Indent(pos, string) => {
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
                EditKind::Indent(pos, string)
            }
        };

        Edit { time: edit.time, kind }
    }

    fn push_edit(&mut self, edit: Edit) {
        self.undo_list.push(edit);
        self.redo_list.clear();
        self.undo = false;
    }

    fn merge_edit(&mut self, edit: Edit) {
        let last_edit = self.undo_list.pop().unwrap();
        let edit = edit.merge(last_edit);
        self.undo_list.push(edit);
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
        let time = self.time();

        for y in pos1.y..=pos2.y {
            let string = unit.repeat(self.rows[y].indent_level);
            if self.rows[y].is_whitespace() {
                if !self.rows[y].is_empty() {
                    let edit = Edit::indent(time, Pos::new(0, y), "".into());
                    let edit = self.process_edit(edit);
                    self.push_edit(edit);
                }
            } else if self.rows[y].indent_part() != string {
                let edit = Edit::indent(time, Pos::new(0, y), string);
                let edit = self.process_edit(edit);
                self.push_edit(edit);
            }
        }
    }

    fn remove_region(&mut self, anchor: Pos) {
        let pos1 = self.cursor.min(anchor);
        let pos2 = self.cursor.max(anchor);
        let edit = Edit::remove(self.time(), pos1, pos2, self.cursor > anchor);
        let edit = self.process_edit(edit);
        self.push_edit(edit);
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
                row.faces[idx..(idx + query.len())].swap_with_slice(&mut faces);
                self.search.matches.push(Match { pos, faces });
            }
        }
        if self.search.matches.is_empty() {
            return;
        }
        self.search.index = if backward {
            self.search
                .matches
                .iter()
                .rposition(|m| m.pos < self.cursor)
                .unwrap_or(self.search.matches.len() - 1)
        } else {
            self.search
                .matches
                .iter()
                .position(|m| m.pos >= self.cursor)
                .unwrap_or(0)
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

        self.search.index = if backward {
            if self.search.index > 0 {
                self.search.index - 1
            } else {
                self.search.matches.len() - 1
            }
        } else {
            if self.search.index < self.search.matches.len() - 1 {
                self.search.index + 1
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

        for m in self.search.matches.iter_mut() {
            let row = &mut self.rows[m.pos.y];
            let idx = row.x_to_idx(m.pos.x);
            row.faces[idx..(idx + m.faces.len())].swap_with_slice(&mut m.faces);
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
        let m = &self.search.matches[self.search.index];
        self.cursor = m.pos;
        self.scroll_center();
    }

    fn highlight_match(&mut self, current: bool) {
        let m = &self.search.matches[self.search.index];
        let row = &mut self.rows[m.pos.y];
        let idx = row.x_to_idx(m.pos.x);
        let face = if current {
            (Fg::CurrentMatch, Bg::CurrentMatch)
        } else {
            (Fg::Match, Bg::Match)
        };
        for i in idx..(idx + m.faces.len()) {
            row.faces[i] = face;
        }
    }
}

impl Buffer {
    pub fn goto_line(&mut self, num: usize) {
        let y = num.saturating_sub(1);
        let y = y.min(self.rows.last_pos().y);
        let pos = Pos::new(0, y);
        if self.anchor.is_some() {
            self.highlight_region(pos);
        }
        self.cursor = pos;
        self.saved_x = pos.x;
        self.scroll_center();
    }

    pub fn mark_whole(&mut self) {
        if let Some(anchor) = self.anchor {
            self.unhighlight_region(anchor);
        }
        let pos = self.rows.last_pos();
        self.anchor = Some(pos);
        self.cursor = pos;
        let pos = Pos::new(0, 0);
        self.highlight_region(pos);
        self.cursor = pos;
        self.saved_x = pos.x;
        self.scroll();
    }

    pub fn save(&mut self) -> io::Result<()> {
        if let Some(file_path) = self.file_path.as_deref() {
            let file = File::create(file_path)?;
            let mut writer = BufWriter::new(file);
            let len = self.rows.len();

            for (i, row) in self.rows.iter_mut().enumerate() {
                writer.write(row.string.as_bytes())?;
                if i < len - 1 {
                    writer.write(b"\n")?;
                }
                row.context = None;
            }

            self.syntax = <dyn Syntax>::detect(Some(file_path));
            self.anchor = None;
            self.last_key = None;
            self.syntax_update(0);

            self.saved_time = self.undo_list.last().map(|e| e.time);
        }
        Ok(())
    }

    pub fn save_as(&mut self, file_path: &str) -> io::Result<()> {
        self.file_path = Some(String::from(file_path));
        self.save()
    }
}
