use std::time::Instant;

pub struct Message {
    pub text: String,
    pub timestamp: Instant,
}

impl Message {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            timestamp: Instant::now(),
        }
    }
}
