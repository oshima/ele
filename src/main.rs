#![allow(clippy::unused_io_amount)]

#[macro_use]
mod color;

mod buffer;
mod canvas;
mod coord;
mod editor;
mod event;
mod face;
mod key;
mod minibuffer;
mod raw_mode;
mod row;
mod rows;
mod syntax;
mod util;

use std::env;
use std::io;

use crate::editor::Editor;
use crate::raw_mode::RawMode;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    let raw_mode = RawMode::new()?;
    raw_mode.enable()?;

    let mut editor = Editor::new(args.get(1).cloned())?;
    editor.run()
}
