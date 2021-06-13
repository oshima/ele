#![allow(clippy::box_vec)]

pub struct UintVec {
    u8_vec: Vec<u8>,
    u16_vec: Option<Box<Vec<u16>>>,
    u32_vec: Option<Box<Vec<u32>>>,
    u64_vec: Option<Box<Vec<u64>>>,
}

impl UintVec {
    pub fn new() -> Self {
        Self {
            u8_vec: Vec::new(),
            u16_vec: None,
            u32_vec: None,
            u64_vec: None,
        }
    }

    pub fn clear(&mut self) {
        self.u8_vec.clear();
        self.u16_vec = None;
        self.u32_vec = None;
        self.u64_vec = None;
    }

    pub fn len(&self) -> usize {
        self.u8_vec.len()
            + self.u16_vec.as_ref().map_or(0, |v| v.len())
            + self.u32_vec.as_ref().map_or(0, |v| v.len())
            + self.u64_vec.as_ref().map_or(0, |v| v.len())
    }

    pub fn at(&self, idx: usize) -> usize {
        let mut idx = idx;

        if idx < self.u8_vec.len() {
            return self.u8_vec[idx] as usize;
        }
        idx -= self.u8_vec.len();

        if let Some(u16_vec) = self.u16_vec.as_ref() {
            if idx < u16_vec.len() {
                return u16_vec[idx] as usize;
            }
            idx -= u16_vec.len();
        }

        if let Some(u32_vec) = self.u32_vec.as_ref() {
            if idx < u32_vec.len() {
                return u32_vec[idx] as usize;
            }
            idx -= u32_vec.len();
        }

        if let Some(u64_vec) = self.u64_vec.as_ref() {
            if idx < u64_vec.len() {
                return u64_vec[idx] as usize;
            }
        }
        panic!()
    }

    pub fn push(&mut self, num: usize) {
        if num <= u8::MAX as usize {
            if let Some(u64_vec) = self.u64_vec.as_mut() {
                u64_vec.push(num as u64);
            } else if let Some(u32_vec) = self.u32_vec.as_mut() {
                u32_vec.push(num as u32);
            } else if let Some(u16_vec) = self.u16_vec.as_mut() {
                u16_vec.push(num as u16);
            } else {
                self.u8_vec.push(num as u8);
            }
        } else if num <= u16::MAX as usize {
            if let Some(u64_vec) = self.u64_vec.as_mut() {
                u64_vec.push(num as u64);
            } else if let Some(u32_vec) = self.u32_vec.as_mut() {
                u32_vec.push(num as u32);
            } else {
                self.u16_vec
                    .get_or_insert(Box::new(Vec::new()))
                    .push(num as u16);
            }
        } else if num <= u32::MAX as usize {
            if let Some(u64_vec) = self.u64_vec.as_mut() {
                u64_vec.push(num as u64);
            } else {
                self.u32_vec
                    .get_or_insert(Box::new(Vec::new()))
                    .push(num as u32);
            }
        } else if num <= u64::MAX as usize {
            self.u64_vec
                .get_or_insert(Box::new(Vec::new()))
                .push(num as u64);
        }
    }
}
