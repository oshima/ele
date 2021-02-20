use std::io;

pub enum Key {
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
    PageUp,
    PageDown,
    Backspace,
    Delete,
    Escape,
    Ctrl(u8),
    Alt(u8),
    Char(char),
}

pub enum KeyError {
    IoError(io::Error),
    Interrupted,
    UnknownKey,
}

impl From<io::Error> for KeyError {
    fn from(error: io::Error) -> Self {
        Self::IoError(error)
    }
}
