mod buffer;
mod coord;
mod editor;
mod key;
mod minibuffer;
mod raw_mode;
mod row;
mod syntax;
mod uint_vec;

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
