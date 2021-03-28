use crate::coord::Pos;
use crate::row::Row;

pub type Rows = Vec<Row>;

pub trait RowsMethods {
    fn max_pos(&self) -> Pos;

    fn prev_pos(&self, pos: Pos) -> Option<Pos>;

    fn next_pos(&self, pos: Pos) -> Option<Pos>;

    fn read_text(&self, pos1: Pos, pos2: Pos) -> String;

    fn insert_text(&mut self, pos: Pos, text: &str) -> Pos;

    fn remove_text(&mut self, pos1: Pos, pos2: Pos) -> String;
}

impl RowsMethods for Rows {
    fn max_pos(&self) -> Pos {
        Pos::new(self[self.len() - 1].max_x(), self.len() - 1)
    }

    fn prev_pos(&self, pos: Pos) -> Option<Pos> {
        if let Some(x) = self[pos.y].prev_x(pos.x) {
            Some(Pos::new(x, pos.y))
        } else if pos.y > 0 {
            Some(Pos::new(self[pos.y - 1].max_x(), pos.y - 1))
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

    fn read_text(&self, pos1: Pos, pos2: Pos) -> String {
        let mut strings = Vec::new();

        for y in pos1.y..=pos2.y {
            let row = &self[y];
            let x1 = if y == pos1.y { pos1.x } else { 0 };
            let x2 = if y == pos2.y { pos2.x } else { row.max_x() };
            strings.push(row.read(x1, x2));
        }
        strings.join("\n")
    }

    fn insert_text(&mut self, pos: Pos, text: &str) -> Pos {
        let strs: Vec<&str> = text.split('\n').collect();

        if strs.len() == 1 {
            let x = self[pos.y].insert_str(pos.x, strs[0]);
            Pos::new(x, pos.y)
        } else {
            let string = self[pos.y].split_off(pos.x);
            let mut rows = self.split_off(pos.y + 1);
            self[pos.y].push_str(&strs[0]);
            self.append(&mut strs[1..].iter().map(|s| Row::new(s.to_string())).collect());
            let pos = self.max_pos();
            self[pos.y].push_str(&string);
            self.append(&mut rows);
            pos
        }
    }

    fn remove_text(&mut self, pos1: Pos, pos2: Pos) -> String {
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
}
