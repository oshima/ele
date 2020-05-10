use std::cmp;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::time::Duration;

use crate::key::Key;
use crate::message::Message;
use crate::row::Row;

pub struct Editor {
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
    pub fn new(stdin: io::Stdin, stdout: io::Stdout) -> Self {
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

    pub fn get_window_size(&mut self) -> io::Result<()> {
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

    pub fn set_message(&mut self, text: &str) {
        self.message = Some(Message::new(text));
    }

    pub fn new_file(&mut self) {
        self.rows.push(Row::new(&[]));
    }

    pub fn open_file(&mut self, filename: &str) -> io::Result<()> {
        self.filename = filename.to_string();

        let file = File::open(filename)?;
        let mut reader = BufReader::new(file);
        let mut buf = String::new();

        let crlf: &[_] = &['\r', '\n'];
        let mut ends_with_lf = false;

        while reader.read_line(&mut buf)? > 0 {
            self.rows
                .push(Row::new(buf.trim_end_matches(crlf).as_bytes()));
            ends_with_lf = buf.ends_with("\n");
            buf.clear();
        }
        if self.rows.is_empty() || ends_with_lf {
            self.rows.push(Row::new(&[]));
        }
        Ok(())
    }

    pub fn refresh_screen(&mut self) -> io::Result<()> {
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

        self.buffer
            .write(&self.filename.as_bytes()[0..filename_len])?;
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

    pub fn read_key(&mut self) -> io::Result<Key> {
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

    pub fn process_keypress(&mut self, key: Key) {
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
                if self.rowoff + self.height < self.rows.len() {
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
