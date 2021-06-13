#[derive(Default)]
pub struct DrawRange {
    start: Option<usize>,
    end: Option<usize>,
}

impl DrawRange {
    pub fn expand_start(&mut self, start: usize) {
        if let Some(n) = self.start {
            self.start = Some(start.min(n));
        } else {
            self.start = Some(start);
        }
    }

    pub fn expand_end(&mut self, end: usize) {
        if let Some(n) = self.end {
            self.end = Some(end.max(n));
        } else {
            self.end = Some(end);
        }
    }

    pub fn expand(&mut self, start: usize, end: usize) {
        self.expand_start(start);
        self.expand_end(end);
    }

    pub fn full_expand_start(&mut self) {
        self.start = Some(usize::MIN);
    }

    pub fn full_expand_end(&mut self) {
        self.end = Some(usize::MAX);
    }

    pub fn full_expand(&mut self) {
        self.full_expand_start();
        self.full_expand_end();
    }

    pub fn clear(&mut self) {
        self.start = None;
        self.end = None;
    }

    pub fn as_tuple(&self) -> Option<(usize, usize)> {
        match (self.start, self.end) {
            (Some(start), Some(end)) => Some((start, end)),
            _ => None,
        }
    }
}
