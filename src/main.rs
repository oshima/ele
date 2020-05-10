mod editor;
mod key;
mod message;
mod raw_mode;
mod row;

use std::env;
use std::io;

use crate::editor::Editor;
use crate::raw_mode::RawMode;

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
        editor.open(&args[1])?;
    }

    editor.looop()
}
