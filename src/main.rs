extern crate termios;

use std::cmp;
use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::time::{Duration, Instant};
use termios::*;

const TAB_WIDTH: usize = 4;

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

struct Row {
    chars: Vec<u8>,
    render: Vec<u8>,
    cx_to_rx: Vec<usize>,
    rx_to_cx: Vec<usize>,
}

impl Row {
    fn new(chars: &[u8]) -> Self {
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

struct Message {
    text: String,
    timestamp: Instant,
}

impl Message {
    fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            timestamp: Instant::now(),
        }
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
    rx: usize,
    rx_cache: usize,
    rowoff: usize,
    coloff: usize,
    filename: String,
    rows: Vec<Row>,
    message: Option<Message>,
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
            rx: 0,
            rx_cache: 0,
            rowoff: 0,
            coloff: 0,
            filename: String::new(),
            rows: vec![],
            message: None,
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
        self.height -= 2; // for status bar and message bar
        Ok(())
    }

    fn set_message(&mut self, text: &str) {
        self.message = Some(Message::new(text));
    }

    fn new_file(&mut self) {
        self.rows.push(Row::new(&[]));
    }

    fn open_file(&mut self, filename: &str) -> io::Result<()> {
        self.filename = filename.to_string();

        let file = File::open(filename)?;
        let mut reader = BufReader::new(file);
        let mut buf = String::new();

        let crlf: &[_] = &['\r', '\n'];
        let mut ends_with_lf = false;

        while reader.read_line(&mut buf)? > 0 {
            self.rows.push(Row::new(buf.trim_end_matches(crlf).as_bytes()));
            ends_with_lf = buf.ends_with("\n");
            buf.clear();
        }
        if self.rows.is_empty() || ends_with_lf {
            self.rows.push(Row::new(&[]));
        }
        Ok(())
    }

    fn refresh_screen(&mut self) -> io::Result<()> {
        self.buffer.write(b"\x1b[?25l")?;
        self.buffer.write(b"\x1b[H")?;

        self.draw_rows()?;
        self.draw_status_bar()?;
        self.draw_message_bar()?;

        self.buffer.write(
            format!(
                "\x1b[{};{}H",
                self.cy - self.rowoff + 1,
                self.rx - self.coloff + 1,
            )
            .as_bytes(),
        )?;
        self.buffer.write(b"\x1b[?25h")?;

        self.stdout.write(&self.buffer)?;
        self.stdout.flush()?;

        self.buffer.clear();
        Ok(())
    }

    fn draw_rows(&mut self) -> io::Result<()> {
        for y in self.rowoff..(self.rowoff + self.height) {
            if y < self.rows.len() {
                let row = &self.rows[y];
                if row.render.len() > self.coloff {
                    let len = cmp::min(row.render.len() - self.coloff, self.width);
                    self.buffer
                        .write(&row.render[self.coloff..(self.coloff + len)])?;
                }
            } else {
                self.buffer.write(b"~")?;

                if self.filename.is_empty() && y == self.height / 3 {
                    let welcome = "Kilo editor";
                    let len = cmp::min(welcome.len(), self.width);
                    let padding = (self.width - welcome.len()) / 2;
                    for _ in 0..padding {
                        self.buffer.write(b" ")?;
                    }
                    self.buffer.write(&welcome.as_bytes()[0..len])?;
                }
            }
            self.buffer.write(b"\x1b[K")?;
            self.buffer.write(b"\r\n")?;
        }
        Ok(())
    }

    fn draw_status_bar(&mut self) -> io::Result<()> {
        let pos = format!("Ln {}, Col {}", self.cy + 1, self.rx + 1);
        let pos_len = cmp::min(pos.len(), self.width);
        let filename_len = cmp::min(self.filename.len(), self.width - pos_len);
        let padding = self.width - pos_len - filename_len;

        self.buffer.write(b"\x1b[7m")?;

        self.buffer.write(&self.filename.as_bytes()[0..filename_len])?;
        for _ in 0..padding {
            self.buffer.write(b" ")?;
        }
        self.buffer.write(&pos.as_bytes()[0..pos_len])?;

        self.buffer.write(b"\x1b[m")?;
        self.buffer.write(b"\r\n")?;
        Ok(())
    }

    fn draw_message_bar(&mut self) -> io::Result<()> {
        if let Some(message) = &self.message {
            if message.timestamp.elapsed() < Duration::from_secs(5) {
                let len = cmp::min(message.text.as_bytes().len(), self.width);
                self.buffer.write(&message.text.as_bytes()[0..len])?;
            }
        }
        self.buffer.write(b"\x1b[K")?;
        Ok(())
    }

    fn read_key(&mut self) -> io::Result<Key> {
        let mut buf = [0; 4];
        while self.stdin.read(&mut buf)? == 0 {}

        match buf {
            [1, ..] => Ok(Key::Home),       // ^A
            [2, ..] => Ok(Key::ArrowLeft),  // ^B
            [5, ..] => Ok(Key::End),        // ^E
            [6, ..] => Ok(Key::ArrowRight), // ^F
            [14, ..] => Ok(Key::ArrowDown), // ^N
            [16, ..] => Ok(Key::ArrowUp),   // ^P
            [17, ..] => Ok(Key::Quit),      // ^Q
            [22, ..] => Ok(Key::PageDown),  // ^V
            [b'\x1b', b'v', ..] => Ok(Key::PageUp),
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
            Key::ArrowLeft => {
                if self.cx > 0 {
                    self.cx -= 1;
                    self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                    self.rx_cache = self.rx;
                } else {
                    if self.cy > 0 {
                        self.cy -= 1;
                        self.cx = self.rows[self.cy].chars.len();
                        self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                        self.rx_cache = self.rx;
                    }
                }
            }
            Key::ArrowRight => {
                if self.cx < self.rows[self.cy].chars.len() {
                    self.cx += 1;
                    self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                    self.rx_cache = self.rx;
                } else {
                    if self.cy < self.rows.len() - 1 {
                        self.cy += 1;
                        self.cx = 0;
                        self.rx = 0;
                        self.rx_cache = 0;
                    }
                }
            }
            Key::ArrowUp => {
                if self.cy > 0 {
                    self.cy -= 1;
                    self.rx = cmp::min(self.rx_cache, self.rows[self.cy].render.len());
                    self.cx = self.rows[self.cy].rx_to_cx[self.rx];
                    self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                }
            }
            Key::ArrowDown => {
                if self.cy < self.rows.len() - 1 {
                    self.cy += 1;
                    self.rx = cmp::min(self.rx_cache, self.rows[self.cy].render.len());
                    self.cx = self.rows[self.cy].rx_to_cx[self.rx];
                    self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                }
            }
            Key::Delete => (),
            Key::Home => {
                self.cx = 0;
                self.rx = 0;
                self.rx_cache = 0;
            }
            Key::End => {
                self.cx = self.rows[self.cy].chars.len();
                self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                self.rx_cache = self.rx;
            }
            Key::PageUp => {
                self.cy -= cmp::min(self.height, self.rowoff);
                self.rowoff -= cmp::min(self.height, self.rowoff);

                self.rx = cmp::min(self.rx_cache, self.rows[self.cy].render.len());
                self.cx = self.rows[self.cy].rx_to_cx[self.rx];
                self.rx = self.rows[self.cy].cx_to_rx[self.cx];
            }
            Key::PageDown => {
                if self.rowoff < self.rows.len() - self.height {
                    self.cy += cmp::min(self.height, self.rows.len() - 1 - self.cy);
                    self.rowoff += self.height;

                    self.rx = cmp::min(self.rx_cache, self.rows[self.cy].render.len());
                    self.cx = self.rows[self.cy].rx_to_cx[self.rx];
                    self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                }
            }
            Key::Quit => unreachable!(),
            Key::Other(_) => (),
        }

        self.scroll();
    }

    fn scroll(&mut self) {
        if self.cy < self.rowoff {
            self.rowoff = self.cy;
        }
        if self.cy >= self.rowoff + self.height {
            self.rowoff = self.cy - self.height + 1;
        }
        if self.rx < self.coloff {
            self.coloff = self.rx;
        }
        if self.rx >= self.coloff + self.width {
            self.coloff = self.rx - self.width + 1;
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
    let args: Vec<String> = env::args().collect();
    let stdin = io::stdin();
    let stdout = io::stdout();

    let raw_mode = RawMode::new(&stdin)?;
    raw_mode.enable()?;

    let mut editor = Editor::new(stdin, stdout);
    editor.get_window_size()?;
    editor.set_message("HELP: Ctrl-Q = quit");

    if args.len() < 2 {
        editor.new_file();
    } else {
        editor.open_file(&args[1])?;
    }

    loop {
        editor.refresh_screen()?;
        match editor.read_key()? {
            Key::Quit => break,
            key => editor.process_keypress(key),
        }
    }
    Ok(())
}
