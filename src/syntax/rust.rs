#![allow(clippy::single_match)]

use std::iter::{self, Chain, Iterator, Peekable, Repeat, Zip};
use std::str::{CharIndices, Chars};

use self::TokenKind::*;
use crate::canvas::Term;
use crate::face::{Bg, Fg};
use crate::row::Row;
use crate::syntax::Syntax;

pub struct Rust;

impl Syntax for Rust {
    fn name(&self) -> &'static str {
        "Rust"
    }

    fn fg_color(&self, term: Term) -> &'static [u8] {
        match term {
            Term::TrueColor => fg_color!(0, 0, 0),
            Term::Color256 => fg_color256!(16),
            Term::Color16 => fg_color16!(black),
        }
    }

    fn bg_color(&self, term: Term) -> &'static [u8] {
        match term {
            Term::TrueColor => bg_color!(222, 165, 132),
            Term::Color256 => bg_color256!(180),
            Term::Color16 => bg_color16!(red),
        }
    }

    fn indent_unit(&self) -> Option<&'static str> {
        Some("    ")
    }

    fn update_rows(&self, rows: &mut [Row]) -> usize {
        let mut ctx_tokens = Vec::new();
        let mut ctx_string = String::new();
        let mut len = 0;

        for (i, row) in rows.iter_mut().enumerate() {
            if i == 0 {
                if row.context.is_none() {
                    row.context = Some(String::new());
                }
            } else {
                if row.context.as_ref() == Some(&ctx_string) {
                    break;
                }
                let context = row.context.get_or_insert(String::new());
                context.clear();
                context.push_str(&ctx_string);
            }

            self.update_row(row, &mut ctx_tokens, &mut ctx_string);
            len += 1;
        }

        len
    }
}

impl Rust {
    fn update_row(&self, row: &mut Row, ctx_tokens: &mut Vec<Token>, ctx_string: &mut String) {
        let mut tokens = Tokens::from(&row.string, row.context.as_deref().unwrap()).peekable();
        let mut prev_token: Option<Token> = None;

        row.faces.clear();
        row.faces
            .resize(row.string.len(), (Fg::Default, Bg::Default));
        row.trailing_bg = Bg::Default;
        row.indent_level = 0;

        ctx_tokens.clear();
        ctx_string.clear();

        while let Some(token) = tokens.next() {
            let fg = match token.kind {
                BlockComment { .. } | LineComment => Fg::Comment,
                CharLit | RawStrLit { .. } | StrLit { .. } => Fg::String,
                Const | Fn | For | Keyword | Let | Mod | Mut | Static => Fg::Keyword,
                Lifetime => Fg::Variable,
                PrimitiveType => Fg::Type,
                Question => Fg::Macro,
                Bang => match prev_token.map(|t| t.kind) {
                    Some(Ident) => Fg::Macro,
                    _ => Fg::Default,
                },
                UpperIdent => match prev_token.map(|t| t.kind) {
                    Some(Const | Static) => Fg::Variable,
                    _ => Fg::Type,
                },
                Ident | RawIdent => match prev_token.map(|t| t.kind) {
                    Some(Fn) => Fg::Function,
                    Some(For | Let | Mut) => Fg::Variable,
                    Some(Mod) => Fg::Module,
                    _ => match tokens.peek().map(|t| t.kind) {
                        Some(Bang) => Fg::Macro,
                        Some(Colon) => Fg::Variable,
                        Some(ColonColon) => Fg::Module,
                        Some(OpenParen { .. }) => Fg::Function,
                        _ => Fg::Default,
                    },
                },
                _ => Fg::Default,
            };

            for i in token.start..token.end {
                row.faces[i].0 = fg;
            }

            match token.kind {
                OpenBrace { newline: true }
                | OpenBracket { newline: true }
                | OpenParen { newline: true } => row.indent_level += 1,
                CloseBrace => match prev_token.map(|t| t.kind) {
                    Some(OpenBrace { newline: true }) => row.indent_level -= 1,
                    _ => (),
                },
                CloseBracket => match prev_token.map(|t| t.kind) {
                    Some(OpenBracket { newline: true }) => row.indent_level -= 1,
                    _ => (),
                },
                CloseParen => match prev_token.map(|t| t.kind) {
                    Some(OpenParen { newline: true }) => row.indent_level -= 1,
                    _ => (),
                },
                _ => (),
            }

            match token.kind {
                BlockComment { open: true, .. }
                | StrLit { open: true }
                | RawStrLit { open: true, .. }
                | OpenBrace { .. }
                | OpenBracket { .. }
                | OpenParen { .. } => ctx_tokens.push(token),
                CloseBrace => match ctx_tokens.last().map(|t| t.kind) {
                    Some(OpenBrace { .. }) => drop(ctx_tokens.pop()),
                    _ => (),
                },
                CloseBracket => match ctx_tokens.last().map(|t| t.kind) {
                    Some(OpenBracket { .. }) => drop(ctx_tokens.pop()),
                    _ => (),
                },
                CloseParen => match ctx_tokens.last().map(|t| t.kind) {
                    Some(OpenParen { .. }) => drop(ctx_tokens.pop()),
                    _ => (),
                },
                _ => (),
            }

            prev_token = Some(token);
        }

        self.convert_context(ctx_tokens, ctx_string);
    }

    fn convert_context(&self, tokens: &[Token], string: &mut String) {
        for token in tokens {
            match token.kind {
                BlockComment { open: true, depth } => {
                    for _ in 0..depth {
                        string.push_str("/*");
                    }
                }
                StrLit { open: true } => {
                    string.push_str("\"");
                }
                RawStrLit {
                    open: true,
                    n_hashes,
                } => {
                    string.push_str("r");
                    for _ in 0..n_hashes {
                        string.push_str("#");
                    }
                    string.push_str("\"");
                }
                OpenBrace { newline } => {
                    string.push_str(if newline { "{\n" } else { "{" });
                }
                OpenBracket { newline } => {
                    string.push_str(if newline { "[\n" } else { "[" });
                }
                OpenParen { newline } => {
                    string.push_str(if newline { "(\n" } else { "(" });
                }
                _ => (),
            }
        }

        if !string.is_empty() && !string.ends_with("\n") {
            string.push_str("\n");
        }
    }
}

#[derive(Clone, Copy)]
struct Token {
    kind: TokenKind,
    start: usize,
    end: usize,
}

#[derive(Clone, Copy)]
enum TokenKind {
    Bang,
    BlockComment { open: bool, depth: usize },
    CharLit,
    CloseBrace,
    CloseBracket,
    CloseParen,
    Colon,
    ColonColon,
    Comma,
    Const,
    Fn,
    For,
    Ident,
    Keyword,
    Let,
    Lifetime,
    LineComment,
    Mod,
    Mut,
    OpenBrace { newline: bool },
    OpenBracket { newline: bool },
    OpenParen { newline: bool },
    PrimitiveType,
    Punct,
    Question,
    RawIdent,
    RawStrLit { open: bool, n_hashes: usize },
    Static,
    StrLit { open: bool },
    UpperIdent,
}

struct Tokens<'a> {
    text: &'a str,
    chars: Peekable<Chain<Zip<Repeat<usize>, Chars<'a>>, CharIndices<'a>>>,
}

impl<'a> Tokens<'a> {
    fn from(text: &'a str, context: &'a str) -> Self {
        Self {
            text,
            chars: iter::repeat(0)
                .zip(context.chars())
                .chain(text.char_indices())
                .peekable(),
        }
    }
}

fn is_delim(ch: char) -> bool {
    ch.is_ascii_whitespace() || ch != '_' && ch.is_ascii_punctuation()
}

impl<'a> Iterator for Tokens<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        let (start, ch) = self.chars.find(|&(_, ch)| !ch.is_ascii_whitespace())?;

        let kind = match ch {
            // comment
            '/' => match self.chars.peek() {
                Some(&(_, '/')) => self.line_comment(),
                Some(&(_, '*')) => self.block_comment(),
                _ => Punct,
            },

            // char or lifetime
            '\'' => match self.chars.peek() {
                Some(&(_, ch)) if is_delim(ch) => self.char_lit(),
                Some(_) => self.char_lit_or_lifetime(),
                None => Punct,
            },

            // string
            '"' => self.str_lit(),

            // raw string or raw identifier
            'r' => match self.chars.peek() {
                Some(&(_, '"')) => self.raw_str_lit(),
                Some(&(_, '#')) => match self.chars.clone().nth(1) {
                    Some((_, ch)) if !is_delim(ch) => self.raw_ident(),
                    _ => self.raw_str_lit(),
                },
                _ => self.ident(start),
            },

            // byte, byte string or raw byte string
            'b' => match self.chars.peek() {
                Some(&(_, '\'')) => {
                    self.chars.next();
                    self.char_lit()
                }
                Some(&(_, '"')) => {
                    self.chars.next();
                    self.str_lit()
                }
                Some(&(_, 'r')) => match self.chars.clone().nth(1) {
                    Some((_, '"' | '#')) => {
                        self.chars.next();
                        self.raw_str_lit()
                    }
                    _ => self.ident(start),
                },
                _ => self.ident(start),
            },

            // punctuation
            ',' => Comma,
            '?' => Question,
            '!' => match self.chars.peek() {
                Some(&(_, '=')) => Punct,
                _ => Bang,
            },
            ':' => match self.chars.next_if(|&(_, ch)| ch == ':') {
                Some(_) => ColonColon,
                _ => Colon,
            },
            '{' => OpenBrace {
                newline: self.chars.next_if(|&(_, ch)| ch == '\n').is_some(),
            },
            '[' => OpenBracket {
                newline: self.chars.next_if(|&(_, ch)| ch == '\n').is_some(),
            },
            '(' => OpenParen {
                newline: self.chars.next_if(|&(_, ch)| ch == '\n').is_some(),
            },
            '}' => CloseBrace,
            ']' => CloseBracket,
            ')' => CloseParen,
            ch if is_delim(ch) => Punct,

            // identifier
            ch if ch.is_ascii_uppercase() => self.upper_ident(),
            _ => self.ident(start),
        };

        let end = self.chars.peek().map_or(self.text.len(), |&(idx, _)| idx);

        Some(Token { kind, start, end })
    }
}

impl<'a> Tokens<'a> {
    fn line_comment(&mut self) -> TokenKind {
        while let Some(_) = self.chars.next() {}
        LineComment
    }

    fn block_comment(&mut self) -> TokenKind {
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
                            return BlockComment { open: false, depth };
                        }
                    }
                    _ => (),
                },
                Some(_) => (),
                None => return BlockComment { open: true, depth },
            }
        }
    }

    fn char_lit(&mut self) -> TokenKind {
        loop {
            match self.chars.next() {
                Some((_, '\'')) | None => break CharLit,
                Some((_, '\\')) => {
                    self.chars.next();
                }
                _ => (),
            }
        }
    }

    fn char_lit_or_lifetime(&mut self) -> TokenKind {
        self.chars.next();
        loop {
            match self.chars.peek() {
                Some(&(_, '\'')) => {
                    self.chars.next();
                    return CharLit;
                }
                Some(&(_, ch)) if !is_delim(ch) => {
                    self.chars.next();
                }
                _ => return Lifetime,
            }
        }
    }

    fn str_lit(&mut self) -> TokenKind {
        loop {
            match self.chars.next() {
                Some((_, '"')) => return StrLit { open: false },
                Some((_, '\\')) => {
                    self.chars.next();
                }
                Some(_) => (),
                None => return StrLit { open: true },
            }
        }
    }

    fn raw_str_lit(&mut self) -> TokenKind {
        let mut n_hashes = 0;
        while let Some(&(_, '#')) = self.chars.peek() {
            self.chars.next();
            n_hashes += 1;
        }
        match self.chars.peek() {
            Some(&(_, '"')) => self.chars.next(),
            _ => return Punct,
        };
        loop {
            match self.chars.next() {
                Some((_, '"')) => {
                    let mut close_hashes = 0;
                    while let Some(&(_, '#')) = self.chars.peek() {
                        if close_hashes == n_hashes {
                            break;
                        }
                        self.chars.next();
                        close_hashes += 1;
                    }
                    if close_hashes == n_hashes {
                        return RawStrLit {
                            open: false,
                            n_hashes,
                        };
                    }
                }
                Some(_) => (),
                None => {
                    return RawStrLit {
                        open: true,
                        n_hashes,
                    }
                }
            }
        }
    }

    fn raw_ident(&mut self) -> TokenKind {
        self.chars.next();
        loop {
            match self.chars.peek() {
                Some(&(_, ch)) if !is_delim(ch) => {
                    self.chars.next();
                }
                _ => return RawIdent,
            }
        }
    }

    fn upper_ident(&mut self) -> TokenKind {
        loop {
            match self.chars.peek() {
                Some(&(_, ch)) if !is_delim(ch) => {
                    self.chars.next();
                }
                _ => return UpperIdent,
            }
        }
    }

    fn ident(&mut self, start: usize) -> TokenKind {
        loop {
            let (end, is_last_char) = match self.chars.peek() {
                Some(&(idx, ch)) => (idx, is_delim(ch)),
                None => (self.text.len(), true),
            };
            if !is_last_char {
                self.chars.next();
                continue;
            }
            return match &self.text[start..end] {
                "const" => Const,
                "fn" => Fn,
                "for" => For,
                "let" => Let,
                "mod" => Mod,
                "mut" => Mut,
                "static" => Static,
                "as" | "async" | "await" | "box" | "break" | "continue" | "crate" | "do"
                | "dyn" | "else" | "enum" | "extern" | "false" | "if" | "impl" | "in" | "loop"
                | "match" | "move" | "priv" | "pub" | "ref" | "return" | "self" | "struct"
                | "super" | "trait" | "true" | "try" | "type" | "use" | "virtual" | "where"
                | "while" | "yield" => Keyword,
                "bool" | "char" | "f32" | "f64" | "i8" | "i16" | "i32" | "i64" | "i128"
                | "isize" | "str" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => {
                    PrimitiveType
                }
                _ => Ident,
            };
        }
    }
}
