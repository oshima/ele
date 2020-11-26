#[derive(Clone, Copy, PartialEq)]
pub enum Hl {
    Default,
    Keyword,
    Type,
    Module,
    Variable,
    Function,
    Macro,
    String,
    Comment,
    Background,
    StatusBar,
}

pub type HlContext = u32;
