#[derive(Clone, Copy, PartialEq)]
pub enum Face {
    Default,
    Keyword,
    Type,
    Module,
    Variable,
    Function,
    Macro,
    String,
    Comment,
    Prompt,
    Background,
    StatusBar,
}
