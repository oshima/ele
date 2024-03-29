use std::io::{self, Write};
use std::ops::Range;

use crate::canvas::Canvas;
use crate::coord::Pos;
use crate::row::Row;

pub type Rows = Vec<Row>;

pub trait RowsMethods {
    fn last_pos(&self) -> Pos;
    fn prev_pos(&self, pos: Pos) -> Option<Pos>;
    fn next_pos(&self, pos: Pos) -> Option<Pos>;
    fn prev_word_pos(&self, pos: Pos) -> Option<Pos>;
    fn next_word_pos(&self, pos: Pos) -> Option<Pos>;
    fn read_str(&self, pos1: Pos, pos2: Pos) -> String;
    fn insert_str(&mut self, pos: Pos, string: &str) -> Pos;
    fn remove_str(&mut self, pos1: Pos, pos2: Pos) -> String;
    fn draw(
        &self,
        canvas: &mut Canvas,
        x_range: Range<usize>,
        y_range: Range<usize>,
    ) -> io::Result<()>;
}

impl RowsMethods for Rows {
    fn last_pos(&self) -> Pos {
        Pos::new(self[self.len() - 1].last_x(), self.len() - 1)
    }

    fn prev_pos(&self, pos: Pos) -> Option<Pos> {
        if let Some(x) = self[pos.y].prev_x(pos.x) {
            Some(Pos::new(x, pos.y))
        } else if pos.y > 0 {
            Some(Pos::new(self[pos.y - 1].last_x(), pos.y - 1))
        } else {
            None
        }
    }

    fn next_pos(&self, pos: Pos) -> Option<Pos> {
        if let Some(x) = self[pos.y].next_x(pos.x) {
            Some(Pos::new(x, pos.y))
        } else if pos.y < self.len() - 1 {
            Some(Pos::new(0, pos.y + 1))
        } else {
            None
        }
    }

    fn prev_word_pos(&self, pos: Pos) -> Option<Pos> {
        if let Some(x) = self[pos.y].prev_word_x(pos.x) {
            return Some(Pos::new(x, pos.y));
        }
        for y in (0..pos.y).rev() {
            if let Some(x) = self[y].last_word_x() {
                return Some(Pos::new(x, y));
            }
        }
        None
    }

    #[allow(clippy::needless_range_loop)]
    fn next_word_pos(&self, pos: Pos) -> Option<Pos> {
        if let Some(x) = self[pos.y].next_word_x(pos.x) {
            return Some(Pos::new(x, pos.y));
        }
        for y in (pos.y + 1)..self.len() {
            if let Some(x) = self[y].first_word_x() {
                return Some(Pos::new(x, y));
            }
        }
        None
    }

    #[allow(clippy::needless_range_loop)]
    fn read_str(&self, pos1: Pos, pos2: Pos) -> String {
        let mut strings = Vec::new();

        for y in pos1.y..=pos2.y {
            let row = &self[y];
            let x1 = if y == pos1.y { pos1.x } else { 0 };
            let x2 = if y == pos2.y { pos2.x } else { row.last_x() };
            strings.push(row.read_str(x1, x2));
        }
        strings.join("\n")
    }

    fn insert_str(&mut self, pos: Pos, string: &str) -> Pos {
        let strings: Vec<&str> = string.split('\n').collect();

        if strings.len() == 1 {
            let x = self[pos.y].insert_str(pos.x, strings[0]);
            Pos::new(x, pos.y)
        } else {
            let string = self[pos.y].split_off(pos.x);
            let mut rows = self.split_off(pos.y + 1);
            self[pos.y].push_str(strings[0]);
            self.append(&mut strings[1..].iter().map(|&s| Row::new(s)).collect());
            let pos = self.last_pos();
            self[pos.y].push_str(&string);
            self.append(&mut rows);
            pos
        }
    }

    fn remove_str(&mut self, pos1: Pos, pos2: Pos) -> String {
        if pos1.y == pos2.y {
            self[pos1.y].remove_str(pos1.x, pos2.x)
        } else {
            let mut removed = vec![self[pos1.y].split_off(pos1.x)];
            let string = self[pos2.y].split_off(pos2.x);
            let mut rows = self.split_off(pos2.y + 1);
            self[pos1.y].push_str(&string);
            removed.append(
                &mut self
                    .split_off(pos1.y + 1)
                    .into_iter()
                    .map(|row| row.string)
                    .collect(),
            );
            self.append(&mut rows);
            removed.join("\n")
        }
    }

    fn draw(
        &self,
        canvas: &mut Canvas,
        x_range: Range<usize>,
        y_range: Range<usize>,
    ) -> io::Result<()> {
        for y in y_range {
            if y < self.len() {
                self[y].draw(canvas, x_range.clone())?;
            }
            canvas.write(b"\x1b[K")?;
            canvas.write(b"\r\n")?;
        }
        Ok(())
    }
}
