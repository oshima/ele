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

    pub fn len(&self) -> usize {
        self.u8_vec.len()
            + self.u16_vec.as_ref().map_or(0, |v| v.len())
            + self.u32_vec.as_ref().map_or(0, |v| v.len())
            + self.u64_vec.as_ref().map_or(0, |v| v.len())
    }

    pub fn clear(&mut self) {
        self.u8_vec.clear();
        self.u16_vec.as_mut().map_or((), |v| v.clear());
        self.u32_vec.as_mut().map_or((), |v| v.clear());
        self.u64_vec.as_mut().map_or((), |v| v.clear());
    }

    pub fn get(&self, i: usize) -> usize {
        let u8_vec_len = self.u8_vec.len();
        if i < u8_vec_len {
            return self.u8_vec[i] as usize;
        }
        let i = i - u8_vec_len;

        let u16_vec_len = self.u16_vec.as_ref().map_or(0, |v| v.len());
        if i < u16_vec_len {
            return self.u16_vec.as_ref().unwrap()[i] as usize;
        }
        let i = i - u16_vec_len;

        let u32_vec_len = self.u32_vec.as_ref().map_or(0, |v| v.len());
        if i < u32_vec_len {
            return self.u32_vec.as_ref().unwrap()[i] as usize;
        }
        let i = i - u32_vec_len;

        let u64_vec_len = self.u64_vec.as_ref().map_or(0, |v| v.len());
        if i < u64_vec_len {
            return self.u64_vec.as_ref().unwrap()[i] as usize;
        }
        panic!()
    }

    pub fn push(&mut self, n: usize) {
        if n <= u8::MAX as usize {
            if self.u64_vec.as_ref().map_or(0, |v| v.len()) > 0 {
                self.u64_vec.as_mut().unwrap().push(n as u64);
            } else if self.u32_vec.as_ref().map_or(0, |v| v.len()) > 0 {
                self.u32_vec.as_mut().unwrap().push(n as u32);
            } else if self.u16_vec.as_ref().map_or(0, |v| v.len()) > 0 {
                self.u16_vec.as_mut().unwrap().push(n as u16);
            } else {
                self.u8_vec.push(n as u8);
            }
        } else if n <= u16::MAX as usize {
            if self.u64_vec.as_ref().map_or(0, |v| v.len()) > 0 {
                self.u64_vec.as_mut().unwrap().push(n as u64);
            } else if self.u32_vec.as_ref().map_or(0, |v| v.len()) > 0 {
                self.u32_vec.as_mut().unwrap().push(n as u32);
            } else {
                self.u16_vec
                    .get_or_insert(Box::new(Vec::new()))
                    .push(n as u16);
            }
        } else if n <= u32::MAX as usize {
            if self.u64_vec.as_ref().map_or(0, |v| v.len()) > 0 {
                self.u64_vec.as_mut().unwrap().push(n as u64);
            } else {
                self.u32_vec
                    .get_or_insert(Box::new(Vec::new()))
                    .push(n as u32);
            }
        } else if n <= u64::MAX as usize {
            self.u64_vec
                .get_or_insert(Box::new(Vec::new()))
                .push(n as u64);
        }
    }
}
