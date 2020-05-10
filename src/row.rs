const TAB_WIDTH: usize = 4;

pub struct Row {
    pub chars: Vec<u8>,
    pub render: Vec<u8>,
    pub cx_to_rx: Vec<usize>,
    pub rx_to_cx: Vec<usize>,
}

impl Row {
    pub fn new(chars: &[u8]) -> Self {
        let mut row = Self {
            chars: chars.to_vec(),
            render: vec![],
            cx_to_rx: vec![],
            rx_to_cx: vec![],
        };
        row.init();
        row
    }

    fn init(&mut self) {
        for (cx, ch) in self.chars.iter().enumerate() {
            self.cx_to_rx.push(self.render.len());

            if *ch == b'\t' {
                for _ in 0..(TAB_WIDTH - self.render.len() % TAB_WIDTH) {
                    self.render.push(b' ');
                    self.rx_to_cx.push(cx);
                }
            } else {
                self.render.push(*ch);
                self.rx_to_cx.push(cx);
            }
        }
        self.cx_to_rx.push(self.render.len());
        self.rx_to_cx.push(self.chars.len());
    }
}
