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
        row.update();
        row
    }

    pub fn insert_char(&mut self, at: usize, ch: u8) {
        self.chars.insert(at, ch);
        self.update();
    }

    pub fn remove_char(&mut self, at: usize) {
        self.chars.remove(at);
        self.update();
    }

    pub fn append_chars(&mut self, chars: &mut Vec<u8>) {
        self.chars.append(chars);
        self.update();
    }

    pub fn remove_chars(&mut self, from: usize, to: usize) {
        let mut chars = self.chars.split_off(to);
        self.chars.truncate(from);
        self.chars.append(&mut chars);
        self.update();
    }

    pub fn truncate_chars(&mut self, at: usize) {
        self.chars.truncate(at);
        self.update();
    }

    pub fn split_chars(&mut self, at: usize) -> Vec<u8> {
        let chars = self.chars.split_off(at);
        self.update();
        chars
    }

    fn update(&mut self) {
        self.cx_to_rx.clear();
        self.rx_to_cx.clear();
        self.render.clear();

        for (cx, ch) in self.chars.iter().enumerate() {
            self.cx_to_rx.push(self.rx_to_cx.len());

            if *ch == b'\t' {
                for _ in 0..(TAB_WIDTH - self.render.len() % TAB_WIDTH) {
                    self.rx_to_cx.push(cx);
                    self.render.push(b' ');
                }
            } else {
                self.rx_to_cx.push(cx);
                self.render.push(*ch);
            }
        }
        let next_cx = self.cx_to_rx.len();
        let next_rx = self.rx_to_cx.len();
        self.cx_to_rx.push(next_rx);
        self.rx_to_cx.push(next_cx);
    }
}
