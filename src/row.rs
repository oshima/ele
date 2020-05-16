const TAB_WIDTH: usize = 4;

pub struct Row {
    pub chars: Vec<u8>,
    pub render: Vec<u8>,
    pub cx_to_rx: Vec<usize>,
    pub rx_to_cx: Vec<usize>,
}

impl Row {
    pub fn new(chars: Vec<u8>) -> Self {
        let mut row = Self {
            chars,
            render: vec![],
            cx_to_rx: vec![],
            rx_to_cx: vec![],
        };
        row.update(0);
        row
    }

    pub fn insert_char(&mut self, cx: usize, ch: u8) {
        self.chars.insert(cx, ch);
        self.update(cx);
    }

    pub fn delete_char(&mut self, cx: usize) {
        self.chars.remove(cx);
        self.update(cx);
    }

    pub fn append_chars(&mut self, chars: &mut Vec<u8>) {
        let len = self.chars.len();
        self.chars.append(chars);
        self.update(len);
    }

    pub fn split_chars(&mut self, cx: usize) -> Vec<u8> {
        let chars = self.chars.split_off(cx);
        self.update(0);
        chars
    }

    pub fn truncate_chars(&mut self, cx: usize) {
        self.chars.truncate(cx);
        self.update(0);
    }

    pub fn truncate_prev_chars(&mut self, cx: usize) {
        let mut chars = self.chars.split_off(cx);
        self.chars.clear();
        self.chars.append(&mut chars);
        self.update(0);
    }

    fn update(&mut self, from_cx: usize) {
        let from_rx = if from_cx == 0 {
            0
        } else {
            self.cx_to_rx[from_cx]
        };

        self.cx_to_rx.truncate(from_cx);
        self.rx_to_cx.truncate(from_rx);
        self.render.truncate(from_rx);

        for (i, ch) in self.chars[from_cx..].iter().enumerate() {
            self.cx_to_rx.push(self.rx_to_cx.len());

            if *ch == b'\t' {
                for _ in 0..(TAB_WIDTH - self.render.len() % TAB_WIDTH) {
                    self.rx_to_cx.push(from_cx + i);
                    self.render.push(b' ');
                }
            } else {
                self.rx_to_cx.push(from_cx + i);
                self.render.push(*ch);
            }
        }

        let next_cx = self.cx_to_rx.len();
        let next_rx = self.rx_to_cx.len();
        self.cx_to_rx.push(next_rx);
        self.rx_to_cx.push(next_cx);
    }
}
