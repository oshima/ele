extern crate termios;

use std::io::{self, Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use termios::*;

struct RawMode {
    fd: RawFd,
    termios: Termios,
}

impl RawMode {
    fn new(stdin: &io::Stdin) -> io::Result<Self> {
        let fd = stdin.as_raw_fd();
        let termios = Termios::from_fd(fd)?;

        Ok(Self { fd, termios })
    }

    fn enable(&self) -> io::Result<()> {
        let mut clone = self.termios.clone();

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

struct Editor {
    stdin: io::Stdin,
    stdout: io::Stdout,
    buffer: Vec<u8>,
    width: usize,
    height: usize,
    cx: usize,
    cy: usize,
}

impl Editor {
    fn new(stdin: io::Stdin, stdout: io::Stdout) -> Self {
        Self {
            stdin,
            stdout,
            buffer: vec![],
            width: 0,
            height: 0,
            cx: 0,
            cy: 0,
        }
    }

    fn get_window_size(&mut self) -> io::Result<()> {
        self.stdout.write(b"\x1b[999C\x1b[999B")?;
        self.stdout.write(b"\x1b[6n")?;
        self.stdout.flush()?;

        let mut buf = [0];
        let mut num = 0;
        while self.stdin.read(&mut buf)? == 1 {
            match buf[0] {
                b'\x1b' | b'[' => (),
                b';' => {
                    self.height = num;
                    num = 0;
                }
                b'R' => {
                    self.width = num;
                    break;
                }
                ch => {
                    num = num * 10 + (ch - b'0') as usize;
                }
            }
        }
        Ok(())
    }

    fn refresh_screen(&mut self) -> io::Result<()> {
        self.buffer.write(b"\x1b[?25l")?;
        self.buffer.write(b"\x1b[H")?;

        for i in 0..self.height {
            self.buffer.write(b"~")?;

            if i == self.height / 3 {
                let welcome = "Kilo editor";
                let padding = (self.width - welcome.len()) / 2;
                for _ in 0..padding {
                    self.buffer.write(b" ")?;
                }
                self.buffer.write(welcome.as_bytes())?;
            }

            self.buffer.write(b"\x1b[K")?;
            if i < self.height - 1 {
                self.buffer.write(b"\r\n")?;
            }
        }

        self.buffer
            .write(format!("\x1b[{};{}H", self.cy + 1, self.cx + 1).as_bytes())?;
        self.buffer.write(b"\x1b[?25h")?;

        self.stdout.write(&self.buffer)?;
        self.stdout.flush()?;

        self.buffer.clear();
        Ok(())
    }

    fn read_key(&mut self) -> io::Result<Key> {
        let mut buf = [0; 4];
        while self.stdin.read(&mut buf)? == 0 {}

        match buf {
            [2, ..] => Ok(Key::ArrowLeft),  // ^B
            [6, ..] => Ok(Key::ArrowRight), // ^F
            [14, ..] => Ok(Key::ArrowDown), // ^N
            [16, ..] => Ok(Key::ArrowUp),   // ^P
            [17, ..] => Ok(Key::Quit),      // ^Q
            [b'\x1b', b'[', b'A', ..] => Ok(Key::ArrowUp),
            [b'\x1b', b'[', b'B', ..] => Ok(Key::ArrowDown),
            [b'\x1b', b'[', b'C', ..] => Ok(Key::ArrowRight),
            [b'\x1b', b'[', b'D', ..] => Ok(Key::ArrowLeft),
            [b'\x1b', b'[', b'F', ..] => Ok(Key::End),
            [b'\x1b', b'[', b'H', ..] => Ok(Key::Home),
            [b'\x1b', b'[', b'O', b'F'] => Ok(Key::End),
            [b'\x1b', b'[', b'O', b'H'] => Ok(Key::Home),
            [b'\x1b', b'[', b'1', b'~'] => Ok(Key::Home),
            [b'\x1b', b'[', b'3', b'~'] => Ok(Key::Delete),
            [b'\x1b', b'[', b'4', b'~'] => Ok(Key::End),
            [b'\x1b', b'[', b'5', b'~'] => Ok(Key::PageUp),
            [b'\x1b', b'[', b'6', b'~'] => Ok(Key::PageDown),
            [b'\x1b', b'[', b'7', b'~'] => Ok(Key::Home),
            [b'\x1b', b'[', b'8', b'~'] => Ok(Key::End),
            [b'\x1b', ..] => Ok(Key::Escape),
            _ => Ok(Key::Other(buf[0])),
        }
    }

    fn process_keypress(&mut self, key: Key) {
        match key {
            Key::Escape => (),
            Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown => {
                self.move_cursor(key)
            }
            Key::Delete => (),
            Key::Home => {
                self.cx = 0;
            }
            Key::End => {
                self.cx = self.width - 1;
            }
            Key::PageUp => {
                for _ in 0..self.height {
                    self.move_cursor(Key::ArrowUp);
                }
            }
            Key::PageDown => {
                for _ in 0..self.height {
                    self.move_cursor(Key::ArrowDown);
                }
            }
            Key::Quit => unreachable!(),
            Key::Other(_) => (),
        }
    }

    fn move_cursor(&mut self, key: Key) {
        match key {
            Key::ArrowLeft => {
                if self.cx > 0 {
                    self.cx -= 1;
                }
            }
            Key::ArrowRight => {
                if self.cx < self.width - 1 {
                    self.cx += 1;
                }
            }
            Key::ArrowUp => {
                if self.cy > 0 {
                    self.cy -= 1;
                }
            }
            Key::ArrowDown => {
                if self.cy < self.height - 1 {
                    self.cy += 1
                }
            }
            _ => unreachable!(),
        }
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        self.stdout.write(b"\x1b[2J").unwrap();
        self.stdout.write(b"\x1b[H").unwrap();
        self.stdout.flush().unwrap();
    }
}

enum Key {
    Escape,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    Quit,
    Other(u8),
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();

    let raw_mode = RawMode::new(&stdin)?;
    raw_mode.enable()?;

    let mut editor = Editor::new(stdin, stdout);
    editor.get_window_size()?;

    loop {
        editor.refresh_screen()?;
        match editor.read_key()? {
            Key::Quit => break,
            key => editor.process_keypress(key),
        }
    }
    Ok(())
}
