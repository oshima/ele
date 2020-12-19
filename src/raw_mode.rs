extern crate termios;

use std::io;
use std::os::unix::io::{AsRawFd, RawFd};
use termios::*;

pub struct RawMode {
    fd: RawFd,
    termios: Termios,
}

impl RawMode {
    pub fn new() -> io::Result<Self> {
        let fd = io::stdin().as_raw_fd();
        let termios = Termios::from_fd(fd)?;

        Ok(Self { fd, termios })
    }

    pub fn enable(&self) -> io::Result<()> {
        let mut clone = self.termios;

        clone.c_iflag &= !(BRKINT | ICRNL | INPCK | ISTRIP | IXON);
        clone.c_oflag &= !(OPOST);
        clone.c_cflag |= CS8;
        clone.c_lflag &= !(ECHO | ICANON | IEXTEN | ISIG);
        clone.c_cc[VMIN] = 0;
        clone.c_cc[VTIME] = 1;

        tcsetattr(self.fd, TCSAFLUSH, &clone)
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        tcsetattr(self.fd, TCSAFLUSH, &self.termios).unwrap();
    }
}
