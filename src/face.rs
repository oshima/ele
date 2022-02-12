#[derive(Clone, Copy, PartialEq)]
pub enum Fg {
    Default,
    Keyword,
    Type,
    Module,
    Variable,
    Function,
    Macro,
    String,
    Number,
    Comment,
    Prompt,
    Match,
    CurrentMatch,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Bg {
    Default,
    Region,
    StatusBar,
    Match,
    CurrentMatch,
}
