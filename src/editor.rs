use signal_hook::{self, consts::signal::SIGWINCH};
use std::io::{self, Read, Write};
use std::str;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::buffer::Buffer;
use crate::canvas::Canvas;
use crate::coord::{Pos, Size};
use crate::key::{Key, KeyError};
use crate::minibuffer::Minibuffer;

#[derive(PartialEq)]
enum State {
    Default,
    Search { backward: bool },
    CtrlX,
    Save,
    Quit,
    Quitted,
}

pub struct Editor {
    stdin: io::Stdin,
    stdout: io::Stdout,
    canvas: Canvas,
    state: State,
    buffer: Buffer,
    minibuffer: Minibuffer,
    clipboard: Vec<String>,
    screen_resized: Arc<AtomicBool>,
}

impl Editor {
    pub fn new(filename: Option<String>) -> io::Result<Self> {
        let mut editor = Self {
            stdin: io::stdin(),
            stdout: io::stdout(),
            canvas: Canvas::new(),
            state: State::Default,
            buffer: Buffer::new(filename)?,
            minibuffer: Minibuffer::new(),
            clipboard: Vec::new(),
            screen_resized: Arc::new(AtomicBool::new(true)),
        };

        // switch to alternate screen buffer
        editor.stdout.write(b"\x1b[?1049h")?;
        editor.stdout.flush()?;

        // detect screen resizing
        signal_hook::flag::register(SIGWINCH, Arc::clone(&editor.screen_resized))?;

        Ok(editor)
    }

    pub fn run(&mut self) -> io::Result<()> {
        while self.state != State::Quitted {
            if self.screen_resized.swap(false, Ordering::Relaxed) {
                self.resize()?;
            }

            self.draw()?;

            match self.read_key() {
                Ok(key) => self.process_keypress(key)?,
                Err(KeyError::IoError(e)) => return Err(e),
                _ => (),
            }
        }
        Ok(())
    }

    fn resize(&mut self) -> io::Result<()> {
        self.stdout.write(b"\x1b[999C\x1b[999B")?;
        self.stdout.write(b"\x1b[6n")?;
        self.stdout.flush()?;

        let mut buf = [0];
        let mut num = 0;
        let (mut w, mut h) = (0, 0);

        while self.stdin.read(&mut buf)? == 1 {
            match buf[0] {
                b'\x1b' | b'[' => (),
                b';' => {
                    h = num;
                    num = 0;
                }
                b'R' => {
                    w = num;
                    break;
                }
                ch => {
                    num = num * 10 + (ch - b'0') as usize;
                }
            }
        }

        self.buffer.resize(Pos::new(0, 0), Size::new(w, h - 2));
        self.minibuffer.resize(Pos::new(0, h - 1), Size::new(w, 1));
        Ok(())
    }

    fn draw(&mut self) -> io::Result<()> {
        self.canvas.write(b"\x1b[?25l")?;

        self.buffer.draw(&mut self.canvas)?;
        self.minibuffer.draw(&mut self.canvas)?;

        match self.state {
            State::Default | State::CtrlX => {
                self.buffer.draw_cursor(&mut self.canvas)?;
            }
            State::Search { .. } | State::Save | State::Quit => {
                self.minibuffer.draw_cursor(&mut self.canvas)?;
            }
            State::Quitted => unreachable!(),
        }

        self.canvas.write(b"\x1b[?25h")?;

        self.stdout.write(self.canvas.as_bytes())?;
        self.canvas.clear();
        self.stdout.flush()
    }

    fn read_key(&mut self) -> Result<Key, KeyError> {
        let mut buf = [0];

        while self.stdin.read(&mut buf)? == 0 {
            if self.screen_resized.load(Ordering::Relaxed) {
                return Err(KeyError::Interrupted);
            }
        }

        match buf[0] {
            0..=26 | 28..=31 => Ok(Key::Ctrl(b'@' + buf[0])),
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
                _ => Err(KeyError::UnknownKey),
            },
            32..=126 => Ok(Key::Char(buf[0] as char)),
            127 => Ok(Key::Backspace),
            _ => match self.read_utf8(buf[0])? {
                Some(ch) => Ok(Key::Char(ch)),
                None => Err(KeyError::UnknownKey),
            },
        }
    }

    fn read_escape_sequence(&mut self) -> io::Result<[u8; 3]> {
        let mut buf = [0; 3];
        self.stdin.read(&mut buf)?; // can result in a timeout
        Ok(buf)
    }

    fn read_utf8(&mut self, first_byte: u8) -> io::Result<Option<char>> {
        let mut buf = [first_byte, 0, 0, 0];

        for i in 1..buf.len() {
            self.stdin.read(&mut buf[i..=i])?;

            if let Ok(s) = str::from_utf8(&buf[0..=i]) {
                return Ok(s.chars().next());
            }
        }
        Ok(None)
    }

    fn process_keypress(&mut self, key: Key) -> io::Result<()> {
        match self.state {
            State::Default => match key {
                Key::Ctrl(b'R') => {
                    self.minibuffer.set_prompt("Search: ");
                    self.state = State::Search { backward: true };
                }
                Key::Ctrl(b'S') => {
                    self.minibuffer.set_prompt("Search: ");
                    self.state = State::Search { backward: false };
                }
                Key::Ctrl(b'X') => {
                    self.minibuffer.set_message("C-x [C-s: save] [C-c: quit]");
                    self.state = State::CtrlX;
                }
                _ => self.buffer.process_keypress(key, &mut self.clipboard),
            },
            State::Search { backward } => match key {
                Key::Ctrl(b'G') => {
                    self.buffer.clear_matches(true);
                    self.minibuffer.set_message("");
                    self.state = State::Default;
                }
                Key::Ctrl(b'J') | Key::Ctrl(b'M') => {
                    self.buffer.clear_matches(false);
                    self.minibuffer.set_message("");
                    self.state = State::Default;
                }
                Key::Ctrl(b'R') => {
                    self.buffer.next_match(true);
                }
                Key::Ctrl(b'S') => {
                    self.buffer.next_match(false);
                }
                _ => {
                    let prev_input = self.minibuffer.get_input();
                    self.minibuffer.process_keypress(key);
                    let input = self.minibuffer.get_input();
                    if input != prev_input {
                        self.buffer.clear_matches(true);
                        self.buffer.search(&input, backward);
                    }
                }
            },
            State::CtrlX => match key {
                Key::Ctrl(b'S') => {
                    if self.buffer.filename.is_none() {
                        self.minibuffer.set_prompt("Save as: ");
                        self.state = State::Save;
                    } else {
                        self.buffer.save()?;
                        self.minibuffer.set_message("Saved");
                        self.state = State::Default;
                    }
                }
                Key::Ctrl(b'C') => {
                    if self.buffer.modified {
                        self.minibuffer
                            .set_prompt("Quit without saving changes? (Y/n): ");
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
        // switch to main screen buffer
        self.stdout.write(b"\x1b[?1049l").unwrap();
        self.stdout.flush().unwrap();
    }
}
