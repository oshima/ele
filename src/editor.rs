use std::cmp;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::time::Duration;

use crate::key::Key;
use crate::message::Message;
use crate::row::Row;

pub struct Editor {
    stdin: io::Stdin,
    stdout: io::Stdout,
    bufout: Vec<u8>,
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
    modified: bool,
    message: Option<Message>,
}

impl Editor {
    pub fn new(stdin: io::Stdin, stdout: io::Stdout) -> Self {
        Self {
            stdin,
            stdout,
            bufout: vec![],
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
            modified: false,
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
        self.rows.push(Row::new(vec![]));
    }

    pub fn open(&mut self, filename: &str) -> io::Result<()> {
        self.filename = filename.to_string();

        let file = File::open(filename)?;
        let mut reader = BufReader::new(file);
        let mut buf = String::new();

        let crlf: &[_] = &['\r', '\n'];
        let mut ends_with_lf = false;

        while reader.read_line(&mut buf)? > 0 {
            self.rows
                .push(Row::new(buf.trim_end_matches(crlf).as_bytes().to_vec()));
            ends_with_lf = buf.ends_with("\n");
            buf.clear();
        }
        if self.rows.is_empty() || ends_with_lf {
            self.rows.push(Row::new(vec![]));
        }
        Ok(())
    }

    pub fn looop(&mut self) -> io::Result<()> {
        let mut quitting = false;

        loop {
            self.refresh_screen()?;
            match self.read_key()? {
                Key::Ctrl(b'q') => {
                    if !self.modified || quitting {
                        break;
                    } else {
                        quitting = true;
                        self.set_message("File has unsaved changes. Press Ctrl-Q to quit.");
                    }
                }
                key => {
                    quitting = false;
                    self.process_keypress(key)?;
                }
            }
        }
        Ok(())
    }

    fn refresh_screen(&mut self) -> io::Result<()> {
        self.bufout.write(b"\x1b[?25l")?;
        self.bufout.write(b"\x1b[H")?;

        self.draw_rows()?;
        self.draw_status_bar()?;
        self.draw_message_bar()?;

        self.bufout.write(
            format!(
                "\x1b[{};{}H",
                self.cy - self.rowoff + 1,
                self.rx - self.coloff + 1,
            )
            .as_bytes(),
        )?;
        self.bufout.write(b"\x1b[?25h")?;

        self.stdout.write(&self.bufout)?;
        self.bufout.clear();

        self.stdout.flush()
    }

    fn draw_rows(&mut self) -> io::Result<()> {
        for y in self.rowoff..(self.rowoff + self.height) {
            if y < self.rows.len() {
                let row = &self.rows[y];
                if row.render.len() > self.coloff {
                    let len = cmp::min(row.render.len() - self.coloff, self.width);
                    self.bufout
                        .write(&row.render[self.coloff..(self.coloff + len)])?;
                }
            } else {
                self.bufout.write(b"~")?;

                if self.filename.is_empty() && y == self.height / 3 {
                    let welcome = "Kilo editor";
                    let len = cmp::min(welcome.len(), self.width);
                    let padding = (self.width - welcome.len()) / 2;
                    for _ in 0..padding {
                        self.bufout.write(b" ")?;
                    }
                    self.bufout.write(&welcome.as_bytes()[0..len])?;
                }
            }
            self.bufout.write(b"\x1b[K")?;
            self.bufout.write(b"\r\n")?;
        }
        Ok(())
    }

    fn draw_status_bar(&mut self) -> io::Result<()> {
        let left = format!(
            "{} {}",
            self.filename,
            if self.modified { "(modified)" } else { "" },
        );
        let right = format!("Ln {}, Col {}", self.cy + 1, self.rx + 1);
        let right_len = cmp::min(right.len(), self.width);
        let left_len = cmp::min(left.len(), self.width - right_len);
        let padding = self.width - left_len - right_len;

        self.bufout.write(b"\x1b[7m")?;

        self.bufout.write(&left.as_bytes()[0..left_len])?;
        for _ in 0..padding {
            self.bufout.write(b" ")?;
        }
        self.bufout.write(&right.as_bytes()[0..right_len])?;

        self.bufout.write(b"\x1b[m")?;
        self.bufout.write(b"\r\n")?;
        Ok(())
    }

    fn draw_message_bar(&mut self) -> io::Result<()> {
        if let Some(message) = &self.message {
            if message.timestamp.elapsed() < Duration::from_secs(5) {
                let len = cmp::min(message.text.as_bytes().len(), self.width);
                self.bufout.write(&message.text.as_bytes()[0..len])?;
            }
        }
        self.bufout.write(b"\x1b[K")?;
        Ok(())
    }

    fn read_key(&mut self) -> io::Result<Key> {
        let mut buf = [0; 4];
        while self.stdin.read(&mut buf)? == 0 {}

        match buf {
            [1..=26, 0, 0, 0] => Ok(Key::Ctrl(b'a' + buf[0] - 1)),
            [127, 0, 0, 0] => Ok(Key::Backspace),
            [b'\x1b', b'a'..=b'z', 0, 0] => Ok(Key::Alt(buf[1])),
            [b'\x1b', b'[', b'A', 0] => Ok(Key::ArrowUp),
            [b'\x1b', b'[', b'B', 0] => Ok(Key::ArrowDown),
            [b'\x1b', b'[', b'C', 0] => Ok(Key::ArrowRight),
            [b'\x1b', b'[', b'D', 0] => Ok(Key::ArrowLeft),
            [b'\x1b', b'[', b'F', 0] => Ok(Key::End),
            [b'\x1b', b'[', b'H', 0] => Ok(Key::Home),
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
            _ => Ok(Key::Plain(buf[0])),
        }
    }

    fn process_keypress(&mut self, key: Key) -> io::Result<()> {
        match key {
            Key::ArrowLeft | Key::Ctrl(b'b')=> {
                if self.cx > 0 {
                    self.cx -= 1;
                    self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                    self.rx_cache = self.rx;
                } else if self.cy > 0 {
                    self.cy -= 1;
                    self.cx = self.rows[self.cy].chars.len();
                    self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                    self.rx_cache = self.rx;
                }
            }
            Key::ArrowRight | Key::Ctrl(b'f') => {
                if self.cx < self.rows[self.cy].chars.len() {
                    self.cx += 1;
                    self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                    self.rx_cache = self.rx;
                } else if self.cy < self.rows.len() - 1 {
                    self.cy += 1;
                    self.cx = 0;
                    self.rx = 0;
                    self.rx_cache = 0;
                }
            }
            Key::ArrowUp | Key::Ctrl(b'p') => {
                if self.cy > 0 {
                    self.cy -= 1;
                    self.rx = cmp::min(self.rx_cache, self.rows[self.cy].render.len());
                    self.cx = self.rows[self.cy].rx_to_cx[self.rx];
                    self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                }
            }
            Key::ArrowDown | Key::Ctrl(b'n') => {
                if self.cy < self.rows.len() - 1 {
                    self.cy += 1;
                    self.rx = cmp::min(self.rx_cache, self.rows[self.cy].render.len());
                    self.cx = self.rows[self.cy].rx_to_cx[self.rx];
                    self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                }
            }
            Key::Home | Key::Ctrl(b'a') => {
                self.cx = 0;
                self.rx = 0;
                self.rx_cache = 0;
            }
            Key::End | Key::Ctrl(b'e') => {
                self.cx = self.rows[self.cy].chars.len();
                self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                self.rx_cache = self.rx;
            }
            Key::PageUp | Key::Alt(b'v') => {
                self.cy -= cmp::min(self.height, self.rowoff);
                self.rowoff -= cmp::min(self.height, self.rowoff);

                self.rx = cmp::min(self.rx_cache, self.rows[self.cy].render.len());
                self.cx = self.rows[self.cy].rx_to_cx[self.rx];
                self.rx = self.rows[self.cy].cx_to_rx[self.cx];
            }
            Key::PageDown | Key::Ctrl(b'v') => {
                if self.rowoff + self.height < self.rows.len() {
                    self.cy += cmp::min(self.height, self.rows.len() - 1 - self.cy);
                    self.rowoff += self.height;

                    self.rx = cmp::min(self.rx_cache, self.rows[self.cy].render.len());
                    self.cx = self.rows[self.cy].rx_to_cx[self.rx];
                    self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                }
            }
            Key::Backspace | Key::Ctrl(b'h') => {
                if self.cx > 0 {
                    self.rows[self.cy].delete_char(self.cx - 1);
                    self.cx -= 1;
                    self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                    self.rx_cache = self.rx;
                    self.modified = true;
                } else if self.cy > 0 {
                    let mut row = self.rows.remove(self.cy);
                    let len = self.rows[self.cy - 1].chars.len();
                    self.rows[self.cy - 1].append_chars(&mut row.chars);
                    self.cy -= 1;
                    self.cx = len;
                    self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                    self.rx_cache = self.rx;
                    self.modified = true;
                }
            }
            Key::Delete | Key::Ctrl(b'd') => {
                if self.cx < self.rows[self.cy].chars.len() {
                    self.rows[self.cy].delete_char(self.cx);
                    self.modified = true;
                } else if self.cy < self.rows.len() - 1 {
                    let mut row = self.rows.remove(self.cy + 1);
                    self.rows[self.cy].append_chars(&mut row.chars);
                    self.modified = true;
                }
            }
            Key::Ctrl(b'i') => {
                self.rows[self.cy].insert_char(self.cx, b'\t');
                self.cx += 1;
                self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                self.rx_cache = self.rx;
                self.modified = true;
            }
            Key::Ctrl(b'j') | Key::Ctrl(b'm') => {
                let chars = self.rows[self.cy].split_chars(self.cx);
                self.rows.insert(self.cy + 1, Row::new(chars));
                self.cy += 1;
                self.cx = 0;
                self.rx = 0;
                self.rx_cache = 0;
                self.modified = true;
            }
            Key::Ctrl(b's') => {
                self.save()?;
                self.modified = false;
                self.set_message("Saved");
            }
            Key::Plain(ch) => {
                self.rows[self.cy].insert_char(self.cx, ch);
                self.cx += 1;
                self.rx = self.rows[self.cy].cx_to_rx[self.cx];
                self.rx_cache = self.rx;
                self.modified = true;
            }
            _ => (),
        }
        self.scroll();
        Ok(())
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

    fn save(&mut self) -> io::Result<()> {
        if self.filename.is_empty() {
            return Ok(());
        }

        let file = File::create(&self.filename)?;
        let mut writer = BufWriter::new(file);

        for (i, row) in self.rows.iter().enumerate() {
            writer.write(&row.chars)?;
            if i < self.rows.len() - 1 {
                writer.write(b"\n")?;
            }
        }
        Ok(())
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        self.stdout.write(b"\x1b[2J").unwrap();
        self.stdout.write(b"\x1b[H").unwrap();
        self.stdout.flush().unwrap();
    }
}
