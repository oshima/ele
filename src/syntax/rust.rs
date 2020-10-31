use std::iter::{Chain, Iterator, Peekable};
use std::str::CharIndices;

use self::TokenKind::*;
use crate::row::Row;
use crate::syntax::{Hl, HlContext, Syntax};

const UNDEFINED: HlContext = 0b00000000;
const NORMAL: HlContext = 0b00000001;
const IN_ATTRIBUTE: HlContext = 0b00000010;
const IN_STRING: HlContext = 0b00000100;
const IN_COMMENT: HlContext = 0b11111000;

struct Token {
    kind: TokenKind,
    start: usize,
    end: usize,
}

#[derive(Clone, Copy)]
enum TokenKind {
    Attribute { open: bool },
    Str { open: bool },
    Comment { depth: u8 },
    Bang,
    Char,
    Colon,
    ColonColon,
    Const,
    Fn,
    For,
    Ident,
    Keyword,
    Let,
    Lifetime,
    Mod,
    Mut,
    Paren,
    PrimitiveType,
    Punct,
    Question,
    Static,
    UpperIdent,
}

struct Tokens<'a> {
    text: &'a str,
    chars: Peekable<Chain<CharIndices<'a>, CharIndices<'a>>>,
}

impl<'a> Tokens<'a> {
    fn from(text: &'a str, context: &'a str) -> Self {
        Self {
            text,
            chars: context.char_indices().chain(text.char_indices()).peekable(),
        }
    }
}

fn is_delim(ch: char) -> bool {
    ch == ' ' || ch != '_' && ch.is_ascii_punctuation()
}

impl<'a> Iterator for Tokens<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        let (start, ch) = self.chars.find(|t| t.1 != ' ')?;

        let kind = match ch {
            // attribute
            '#' => match self.chars.peek() {
                Some(&(_, '[')) => {
                    let open = self.chars.find(|t| t.1 == ']').is_none();
                    Attribute { open }
                }
                Some(&(_, '!')) => {
                    self.chars.next();
                    match self.chars.peek() {
                        Some(&(_, '[')) => {
                            let open = self.chars.find(|t| t.1 == ']').is_none();
                            Attribute { open }
                        }
                        _ => Punct,
                    }
                }
                _ => Punct,
            },

            // string literal
            '"' => loop {
                match self.chars.next() {
                    Some((_, '"')) => break Str { open: false },
                    Some((_, '\\')) => {
                        self.chars.next();
                    }
                    Some(_) => (),
                    None => break Str { open: true },
                }
            },

            // comment
            '/' => match self.chars.peek() {
                Some(&(_, '/')) => {
                    while let Some(_) = self.chars.next() {}
                    Comment { depth: 0 }
                }
                Some(&(_, '*')) => {
                    self.chars.next();
                    let mut depth = 1;
                    loop {
                        match self.chars.next() {
                            Some((_, '/')) => match self.chars.peek() {
                                Some(&(_, '*')) => {
                                    self.chars.next();
                                    depth += 1;
                                }
                                _ => (),
                            },
                            Some((_, '*')) => match self.chars.peek() {
                                Some(&(_, '/')) => {
                                    self.chars.next();
                                    depth -= 1;
                                    if depth == 0 {
                                        break Comment { depth };
                                    }
                                }
                                _ => (),
                            },
                            Some(_) => (),
                            None => break Comment { depth },
                        }
                    }
                }
                _ => Punct,
            },

            // char literal or lifetime
            '\'' => match self.chars.peek() {
                Some(&(_, ch)) if is_delim(ch) => loop {
                    match self.chars.next() {
                        Some((_, '\'')) | None => break Char,
                        Some((_, '\\')) => {
                            self.chars.next();
                        }
                        _ => (),
                    }
                },
                Some(_) => {
                    self.chars.next();
                    loop {
                        match self.chars.peek() {
                            Some(&(_, '\'')) => {
                                self.chars.next();
                                break Char;
                            }
                            Some(&(_, ch)) if !is_delim(ch) => {
                                self.chars.next();
                            }
                            _ => break Lifetime,
                        }
                    }
                }
                None => Punct,
            },

            // punctuation
            '(' => Paren,
            '?' => Question,
            '!' => match self.chars.peek() {
                Some(&(_, '=')) => Punct,
                _ => Bang,
            },
            ':' => match self.chars.peek() {
                Some(&(_, ':')) => {
                    self.chars.next();
                    ColonColon
                }
                _ => Colon,
            },
            ch if is_delim(ch) => Punct,

            // keyword or identifier
            ch => loop {
                let (end, is_last_char) = match self.chars.peek() {
                    Some(&(idx, ch)) => (idx, is_delim(ch)),
                    None => (self.text.len(), true),
                };
                if !is_last_char {
                    self.chars.next();
                    continue;
                }
                if ch.is_ascii_uppercase() {
                    break UpperIdent;
                }
                break match &self.text[start..end] {
                    "const" => Const,
                    "fn" => Fn,
                    "for" => For,
                    "let" => Let,
                    "mod" => Mod,
                    "mut" => Mut,
                    "static" => Static,
                    "as" | "async" | "await" | "box" | "break" | "continue" | "crate" | "do"
                    | "dyn" | "else" | "enum" | "extern" | "false" | "if" | "impl" | "in"
                    | "loop" | "match" | "move" | "priv" | "pub" | "ref" | "return" | "self"
                    | "struct" | "super" | "trait" | "true" | "try" | "type" | "use"
                    | "virtual" | "where" | "while" | "yield" => Keyword,
                    "bool" | "char" | "f32" | "f64" | "i8" | "i16" | "i32" | "i64" | "i128"
                    | "isize" | "str" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => {
                        PrimitiveType
                    }
                    _ => Ident,
                };
            },
        };

        let end = self.chars.peek().map_or(self.text.len(), |t| t.0);

        Some(Token { kind, start, end })
    }
}

pub struct Rust;

impl Syntax for Rust {
    fn name(&self) -> &'static str {
        "Rust"
    }

    fn highlight(&self, rows: &mut [Row]) {
        let mut new_context = UNDEFINED;

        for (i, row) in rows.iter_mut().enumerate() {
            if i == 0 {
                if row.hl_context == UNDEFINED {
                    row.hl_context = NORMAL;
                }
            } else {
                if row.hl_context == new_context {
                    break;
                }
                row.hl_context = new_context;
            }
            new_context = self.highlight_row(row);
        }
    }
}

impl Rust {
    fn highlight_row(&self, row: &mut Row) -> HlContext {
        row.hls.clear();
        row.hls.resize(row.render.len(), Hl::Default);

        let string;
        let context = if row.hl_context & IN_COMMENT != 0 {
            let depth = row.hl_context >> IN_COMMENT.trailing_zeros();
            string = "/*".repeat(depth as usize);
            &string
        } else if row.hl_context & IN_ATTRIBUTE != 0 {
            "#["
        } else if row.hl_context & IN_STRING != 0 {
            "\""
        } else {
            ""
        };

        let mut tokens = Tokens::from(&row.render, context).peekable();
        let mut prev_token: Option<Token> = None;

        while let Some(token) = tokens.next() {
            let hl = match token.kind {
                Attribute { .. } => Hl::Macro,
                Char | Str { .. } => Hl::String,
                Comment { .. } => Hl::Comment,
                Const | Fn | For | Keyword | Let | Mod | Mut | Static => Hl::Keyword,
                Lifetime => Hl::Variable,
                PrimitiveType => Hl::Type,
                Question => Hl::Macro,
                Bang => match prev_token.map(|t| t.kind) {
                    Some(Ident) => Hl::Macro,
                    _ => Hl::Default,
                },
                UpperIdent => match prev_token.map(|t| t.kind) {
                    Some(Const) | Some(Static) => Hl::Variable,
                    _ => Hl::Type,
                },
                Ident => match prev_token.map(|t| t.kind) {
                    Some(Fn) => Hl::Function,
                    Some(For) | Some(Let) | Some(Mut) => Hl::Variable,
                    Some(Mod) => Hl::Module,
                    _ => match tokens.peek().map(|t| t.kind) {
                        Some(Bang) => Hl::Macro,
                        Some(Colon) => Hl::Variable,
                        Some(ColonColon) => Hl::Module,
                        Some(Paren) => Hl::Function,
                        _ => Hl::Default,
                    },
                },
                _ => Hl::Default,
            };

            for i in token.start..token.end {
                row.hls[i] = hl;
            }

            prev_token = Some(token);
        }

        match prev_token.map(|t| t.kind) {
            Some(Attribute { open: true }) => IN_ATTRIBUTE,
            Some(Str { open: true }) => IN_STRING,
            Some(Comment { depth }) if depth > 0 => {
                depth << IN_COMMENT.trailing_zeros()
            },
            _ => NORMAL,
        }
    }
}
