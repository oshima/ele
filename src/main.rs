extern crate termios;

use std::io::{self, Read};
use std::os::unix::io::{AsRawFd, RawFd};
use termios::*;

struct RawMode {
    fd: RawFd,
    termios: Termios,
}

impl RawMode {
    fn new(fd: RawFd) -> Self {
        Self {
            fd,
            termios: Termios::from_fd(fd).unwrap(),
        }
    }

    fn enable(&self) {
        let mut clone = self.termios.clone();

        clone.c_iflag &= !(BRKINT | ICRNL | INPCK | ISTRIP | IXON);
        clone.c_oflag &= !(OPOST);
        clone.c_cflag |= CS8;
        clone.c_lflag &= !(ECHO | ICANON | IEXTEN | ISIG);
        clone.c_cc[VMIN] = 0;
        clone.c_cc[VTIME] = 1;

        tcsetattr(self.fd, TCSAFLUSH, &clone).unwrap();
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        tcsetattr(self.fd, TCSAFLUSH, &self.termios).unwrap();
    }
}

fn main() {
    let mut stdin = io::stdin();
    let raw_mode = RawMode::new(stdin.as_raw_fd());

    raw_mode.enable();

    loop {
        let mut buffer = [0];
        stdin.read(&mut buffer).unwrap();

        let ch = buffer[0];

        if ch.is_ascii_control() {
            print!("{}\r\n", ch);
        } else {
            print!("{} ({})\r\n", ch, ch as char);
        }
        if ch == 'q' as u8 {
            break;
        }
    }
}
