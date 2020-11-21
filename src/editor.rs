use std::io::{self, Read, Write};
use std::str;

use crate::buffer::Buffer;
use crate::coord::{Pos, Size};
use crate::key::Key;
use crate::minibuffer::Minibuffer;

#[derive(PartialEq)]
enum State {
    Default,
    CtrlX,
    Save,
    Quit,
    Quitted,
}

pub struct Editor {
    stdin: io::Stdin,
    stdout: io::Stdout,
    canvas: Vec<u8>,
    state: State,
    buffer: Buffer,
    minibuffer: Minibuffer,
}

impl Editor {
    pub fn new(filename: Option<String>) -> io::Result<Self> {
        let mut editor = Self {
            stdin: io::stdin(),
            stdout: io::stdout(),
            canvas: Vec::new(),
            state: State::Default,
            buffer: Buffer::new(filename)?,
            minibuffer: Minibuffer::new(),
        };
        let Size { w, h } = editor.get_window_size()?;
        editor.buffer.pos = Pos::new(0, 0);
        editor.buffer.size = Size::new(w, h - 2);
        editor.minibuffer.pos = Pos::new(0, h - 1);
        editor.minibuffer.size = Size::new(w, 1);
        Ok(editor)
    }

    fn get_window_size(&mut self) -> io::Result<Size> {
        self.stdout.write(b"\x1b[999C\x1b[999B")?;
        self.stdout.write(b"\x1b[6n")?;
        self.stdout.flush()?;

        let mut buf = [0];
        let mut num = 0;
        let mut size = Size::new(0, 0);

        while self.stdin.read(&mut buf)? == 1 {
            match buf[0] {
                b'\x1b' | b'[' => (),
                b';' => {
                    size.h = num;
                    num = 0;
                }
                b'R' => {
                    size.w = num;
                    break;
                }
                ch => {
                    num = num * 10 + (ch - b'0') as usize;
                }
            }
        }
        Ok(size)
    }

    pub fn run(&mut self) -> io::Result<()> {
        while self.state != State::Quitted {
            self.refresh_screen()?;

            let key = self.read_key()?;
            self.process_keypress(key)?;
        }
        Ok(())
    }

    fn refresh_screen(&mut self) -> io::Result<()> {
        self.canvas.write(b"\x1b[?25l")?;

        match self.state {
            State::Default => {
                self.buffer.draw(&mut self.canvas)?;
                self.minibuffer.draw(&mut self.canvas)?;
                self.buffer.draw_cursor(&mut self.canvas)?;
            }
            State::CtrlX => {
                self.minibuffer.draw(&mut self.canvas)?;
                self.buffer.draw_cursor(&mut self.canvas)?;
            }
            State::Save | State::Quit => {
                self.minibuffer.draw(&mut self.canvas)?;
                self.minibuffer.draw_cursor(&mut self.canvas)?;
            }
            State::Quitted => unreachable!(),
        }

        self.canvas.write(b"\x1b[?25h")?;

        self.stdout.write(&self.canvas)?;
        self.canvas.clear();
        self.stdout.flush()
    }

    fn read_key(&mut self) -> io::Result<Key> {
        let byte = self.read_byte()?;

        match byte {
            0..=26 | 28..=31 => Ok(Key::Ctrl(b'@' + byte)),
            27 => match self.read_escape_sequence()? {
                [0, 0, 0] => Ok(Key::Escape),
                [b, 0, 0] => Ok(Key::Alt(b)),
                [b'[', b'A', 0] => Ok(Key::ArrowUp),
                [b'[', b'B', 0] => Ok(Key::ArrowDown),
                [b'[', b'C', 0] => Ok(Key::ArrowRight),
                [b'[', b'D', 0] => Ok(Key::ArrowLeft),
                [b'[', b'F', 0] => Ok(Key::End),
                [b'[', b'H', 0] => Ok(Key::Home),
                [b'[', b'O', b'F'] => Ok(Key::End),
                [b'[', b'O', b'H'] => Ok(Key::Home),
                [b'[', b'1', b'~'] => Ok(Key::Home),
                [b'[', b'3', b'~'] => Ok(Key::Delete),
                [b'[', b'4', b'~'] => Ok(Key::End),
                [b'[', b'5', b'~'] => Ok(Key::PageUp),
                [b'[', b'6', b'~'] => Ok(Key::PageDown),
                [b'[', b'7', b'~'] => Ok(Key::Home),
                [b'[', b'8', b'~'] => Ok(Key::End),
                _ => Ok(Key::Unknown),
            },
            32..=126 => Ok(Key::Char(byte as char)),
            127 => Ok(Key::Backspace),
            _ => match self.read_utf8(byte)? {
                Some(ch) => Ok(Key::Char(ch)),
                None => Ok(Key::Unknown),
            },
        }
    }

    #[inline]
    fn read_byte(&mut self) -> io::Result<u8> {
        let mut buf = [0];
        while self.stdin.read(&mut buf)? == 0 {}
        Ok(buf[0])
    }

    #[inline]
    fn read_escape_sequence(&mut self) -> io::Result<[u8; 3]> {
        let mut buf = [0; 3];
        self.stdin.read(&mut buf)?; // can result in a timeout
        Ok(buf)
    }

    fn read_utf8(&mut self, first_byte: u8) -> io::Result<Option<char>> {
        let mut buf = [first_byte, 0, 0, 0];

        for i in 1..buf.len() {
            buf[i] = self.read_byte()?;

            if let Ok(s) = str::from_utf8(&buf[0..=i]) {
                return Ok(s.chars().next());
            }
        }
        Ok(None)
    }

    fn process_keypress(&mut self, key: Key) -> io::Result<()> {
        match self.state {
            State::Default => match key {
                Key::Ctrl(b'X') => {
                    self.minibuffer.set_message("C-x [C-s: save] [C-c: quit]");
                    self.state = State::CtrlX;
                }
                _ => self.buffer.process_keypress(key),
            },
            State::CtrlX => match key {
                Key::Ctrl(b'S') => {
                    if self.buffer.filename.is_none() {
                        self.minibuffer.set_prompt("Save as: ");
                        self.state = State::Save;
                    } else {
                        self.buffer.save()?;
                        self.minibuffer.set_message("Saved");
                    }
                }
                Key::Ctrl(b'C') => {
                    if self.buffer.modified {
                        self.minibuffer.set_prompt("Quit without saving? (Y/n): ");
                        self.state = State::Quit;
                    } else {
                        self.state = State::Quitted;
                    }
                }
                _ => {
                    self.minibuffer.set_message("");
                    self.state = State::Default;
                }
            },
            State::Save => match key {
                Key::Ctrl(b'G') => {
                    self.minibuffer.set_message("");
                    self.state = State::Default;
                }
                Key::Ctrl(b'J') | Key::Ctrl(b'M') => {
                    let input = self.minibuffer.get_input();
                    self.buffer.filename = Some(input);
                    self.buffer.save()?;
                    self.minibuffer.set_message("");
                    self.state = State::Default;
                }
                _ => self.minibuffer.process_keypress(key),
            },
            State::Quit => match key {
                Key::Ctrl(b'G') => {
                    self.minibuffer.set_message("");
                    self.state = State::Default;
                }
                Key::Ctrl(b'J') | Key::Ctrl(b'M') => {
                    let input = self.minibuffer.get_input();
                    if input.is_empty() || input.to_lowercase() == "y" {
                        self.state = State::Quitted;
                    } else {
                        self.minibuffer.set_message("");
                        self.state = State::Default;
                    }
                }
                _ => self.minibuffer.process_keypress(key),
            },
            State::Quitted => unreachable!(),
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
