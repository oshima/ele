use std::iter::{self, Chain, Iterator, Peekable, Repeat, Zip};
use std::str::{CharIndices, Chars};

use self::TokenKind::*;
use crate::canvas::Term;
use crate::face::{Bg, Fg};
use crate::row::Row;
use crate::syntax::Syntax;

pub struct Rust;

impl Syntax for Rust {
    fn matches(file_name: &str) -> bool {
        file_name.ends_with(".rs")
    }

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
        let mut context_v = Vec::new();
        let mut context_s = String::new();

        for (i, row) in rows.iter_mut().enumerate() {
            if i == 0 {
                if row.context.is_none() {
                    row.context = Some(String::new());
                }
            } else {
                if row.context.as_ref() == Some(&context_s) {
                    return i;
                }
                let context = row.context.get_or_insert(String::new());
                context.clear();
                context.push_str(&context_s);
            }

            context_v.clear();
            context_s.clear();
            self.update_row(row, &mut context_v, &mut context_s);
        }

        rows.len()
    }
}

impl Rust {
    #![allow(clippy::single_match)]
    fn update_row(&self, row: &mut Row, context_v: &mut Vec<TokenKind>, context_s: &mut String) {
        let mut tokens = Tokens::from(&row.string, row.context.as_deref().unwrap()).peekable();
        let mut prev_token: Option<Token> = None;

        row.faces.clear();
        row.faces
            .resize(row.string.len(), (Fg::Default, Bg::Default));
        row.indent_level = 0;

        while let Some(token) = tokens.next() {
            // Highlight
            let fg = match token.kind {
                Bang => match prev_token.map(|t| t.kind) {
                    Some(Ident) => Fg::Macro,
                    _ => Fg::Default,
                },
                BlockComment { .. } | LineComment => Fg::Comment,
                CharLit | RawStrLit { .. } | StrLit { .. } => Fg::String,
                Const | Fn | For | Keyword | Let | Mod | Mut | Static | Where { .. } => Fg::Keyword,
                Ident => match prev_token.map(|t| t.kind) {
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
                Lifetime => Fg::Variable,
                NumberLit => Fg::Number,
                PrimitiveType => Fg::Type,
                Question => Fg::Macro,
                UpperIdent => match prev_token.map(|t| t.kind) {
                    Some(Const | Static) => Fg::Variable,
                    _ => Fg::Type,
                },
                _ => Fg::Default,
            };

            for i in token.start..token.end {
                row.faces[i].0 = fg;
            }

            // Indent
            match token.kind {
                Expr { lf: true }
                | OpenAttribute { lf: true }
                | OpenBrace { lf: true }
                | OpenBracket { lf: true }
                | OpenParen { lf: true }
                | Where { lf: true } => {
                    row.indent_level += 1;
                }
                _ => (),
            }

            if let Some(0) = prev_token.map(|t| t.end) {
                match token.kind {
                    CloseBrace => match context_v[..] {
                        [.., OpenBrace { lf }, Expr { lf: true }] => {
                            row.indent_level -= if lf { 2 } else { 1 };
                        }
                        [.., OpenBrace { lf: true }] => {
                            row.indent_level -= 1;
                        }
                        _ => (),
                    },
                    CloseBracket => match context_v[..] {
                        [.., OpenAttribute { lf }, Expr { lf: true }] => {
                            row.indent_level -= if lf { 2 } else { 1 };
                        }
                        [.., OpenAttribute { lf: true }] => {
                            row.indent_level -= 1;
                        }
                        [.., OpenBracket { lf }, Expr { lf: true }] => {
                            row.indent_level -= if lf { 2 } else { 1 };
                        }
                        [.., OpenBracket { lf: true }] => {
                            row.indent_level -= 1;
                        }
                        _ => (),
                    },
                    CloseParen => match context_v[..] {
                        [.., OpenParen { lf }, Expr { lf: true }] => {
                            row.indent_level -= if lf { 2 } else { 1 };
                        }
                        [.., OpenParen { lf: true }] => {
                            row.indent_level -= 1;
                        }
                        _ => (),
                    },
                    OpenBrace { lf: false } => match context_v[..] {
                        [.., Where { lf }, Expr { lf: true }] => {
                            row.indent_level -= if lf { 2 } else { 1 };
                        }
                        [.., Expr { lf: true } | Where { lf: true }] => {
                            row.indent_level -= 1;
                        }
                        _ => (),
                    },
                    Or => match context_v[..] {
                        [.., Expr { lf: true }] => {
                            row.indent_level -= 1;
                        }
                        _ => (),
                    },
                    Where { lf: false } => match context_v[..] {
                        [.., Expr { lf: true }] => {
                            row.indent_level -= 1;
                        }
                        _ => (),
                    },
                    _ => (),
                }
            }

            // Derive the context of the next row
            match token.kind {
                BlockComment { open: true, .. }
                | Expr { .. }
                | OpenAttribute { .. }
                | OpenBracket { .. }
                | OpenParen { .. }
                | RawStrLit { open: true, .. }
                | StrLit { open: true } => {
                    context_v.push(token.kind);
                }
                OpenBrace { .. } => match context_v[..] {
                    [.., Where { .. }, Expr { .. }] => {
                        context_v.pop();
                        context_v.pop();
                        context_v.push(token.kind);
                    }
                    [.., Expr { .. } | Where { .. }] => {
                        context_v.pop();
                        context_v.push(token.kind);
                    }
                    _ => context_v.push(token.kind),
                },
                Where { .. } => match context_v[..] {
                    [.., Expr { .. }] => {
                        context_v.pop();
                        context_v.push(token.kind);
                    }
                    _ => context_v.push(token.kind),
                },
                CloseBrace => match context_v[..] {
                    [.., OpenBrace { .. }, Expr { .. }] => {
                        context_v.pop();
                        context_v.pop();
                    }
                    [.., OpenBrace { .. }] => {
                        context_v.pop();
                    }
                    _ => (),
                },
                CloseBracket => match context_v[..] {
                    [.., OpenAttribute { .. }, Expr { .. }] => {
                        context_v.pop();
                        context_v.pop();
                    }
                    [.., OpenAttribute { .. }] => {
                        context_v.pop();
                    }
                    [.., OpenBracket { .. }, Expr { .. }] => {
                        context_v.pop();
                        context_v.pop();
                        if !matches!(context_v[..], [.., Expr { .. }]) {
                            context_v.push(Expr { lf: false });
                        }
                    }
                    [.., OpenBracket { .. }] => {
                        context_v.pop();
                        if !matches!(context_v[..], [.., Expr { .. }]) {
                            context_v.push(Expr { lf: false });
                        }
                    }
                    _ => (),
                },
                CloseParen => match context_v[..] {
                    [.., OpenParen { .. }, Expr { .. }] => {
                        context_v.pop();
                        context_v.pop();
                        if !matches!(context_v[..], [.., Expr { .. }]) {
                            context_v.push(Expr { lf: false });
                        }
                    }
                    [.., OpenParen { .. }] => {
                        context_v.pop();
                        if !matches!(context_v[..], [.., Expr { .. }]) {
                            context_v.push(Expr { lf: false });
                        }
                    }
                    _ => (),
                },
                Comma => match context_v[..] {
                    [.., Expr { .. }] => {
                        context_v.pop();
                    }
                    _ => (),
                },
                Semi => match context_v[..] {
                    [.., Where { .. }, Expr { .. }] => {
                        context_v.pop();
                        context_v.pop();
                    }
                    [.., Expr { .. } | Where { .. }] => {
                        context_v.pop();
                    }
                    _ => (),
                },
                LineComment | BlockComment { open: false, .. } => (),
                _ => match context_v[..] {
                    [.., Expr { .. }] => (),
                    _ => context_v.push(Expr { lf: false }),
                },
            }

            prev_token = Some(token);
        }

        if let Some(
            Expr { lf }
            | OpenAttribute { lf }
            | OpenBrace { lf }
            | OpenBracket { lf }
            | OpenParen { lf }
            | Where { lf },
        ) = context_v.last_mut()
        {
            *lf = true;
        }

        self.convert_context(context_v, context_s);
    }

    #[rustfmt::skip]
    fn convert_context(&self, slice: &[TokenKind], string: &mut String) {
        for token_kind in slice {
            match *token_kind {
                BlockComment { open: true, depth } => {
                    for _ in 0..depth {
                        string.push_str("/*");
                    }
                }
                Expr { lf } => {
                    string.push_str(if lf { "\0e\n" } else { "\0e" });
                }
                OpenAttribute { lf } => {
                    string.push_str(if lf { "#[\n" } else { "#[" });
                }
                OpenBrace { lf } => {
                    string.push_str(if lf { "{\n" } else { "{" });
                }
                OpenBracket { lf } => {
                    string.push_str(if lf { "[\n" } else { "[" });
                }
                OpenParen { lf } => {
                    string.push_str(if lf { "(\n" } else { "(" });
                }
                RawStrLit { open: true, n_hashes } => {
                    string.push('r');
                    for _ in 0..n_hashes {
                        string.push('#');
                    }
                    string.push('"');
                }
                StrLit { open: true } => {
                    string.push('"');
                }
                Where { lf } => {
                    string.push_str(if lf { "\0w\n" } else { "\0w" });
                }
                _ => (),
            }
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
    Expr { lf: bool },
    Fn,
    For,
    Ident,
    Keyword,
    Let,
    Lifetime,
    LineComment,
    Mod,
    Mut,
    NumberLit,
    OpenAttribute { lf: bool },
    OpenBrace { lf: bool },
    OpenBracket { lf: bool },
    OpenParen { lf: bool },
    Or,
    PrimitiveType,
    Punct,
    Question,
    RawStrLit { open: bool, n_hashes: usize },
    Semi,
    Static,
    StrLit { open: bool },
    UpperIdent,
    Where { lf: bool },
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
                Some(&(_, 'r')) => {
                    self.chars.next();
                    match self.chars.peek() {
                        Some((_, '"' | '#')) => self.raw_str_lit(),
                        _ => self.ident(start),
                    }
                }
                _ => self.ident(start),
            },

            // number
            '0' => match self.chars.peek() {
                Some(&(_, 'b')) => self.n_ary_lit(2),
                Some(&(_, 'o')) => self.n_ary_lit(8),
                Some(&(_, 'x')) => self.n_ary_lit(16),
                _ => self.number_lit(),
            },
            '1'..='9' => self.number_lit(),

            // punctuation
            ',' => Comma,
            '?' => Question,
            ';' => Semi,
            '!' => match self.chars.next_if(|&(_, ch)| ch == '=') {
                Some(_) => Punct,
                _ => Bang,
            },
            ':' => match self.chars.next_if(|&(_, ch)| ch == ':') {
                Some(_) => ColonColon,
                _ => Colon,
            },
            '|' => match self.chars.next_if(|&(_, ch)| ch == '|') {
                Some(_) => Punct,
                _ => Or,
            },
            '#' => {
                self.chars.next_if(|&(_, ch)| ch == '!');
                match self.chars.next_if(|&(_, ch)| ch == '[') {
                    Some(_) => OpenAttribute {
                        lf: self.chars.next_if(|&(_, ch)| ch == '\n').is_some(),
                    },
                    _ => Punct,
                }
            }
            '{' => OpenBrace {
                lf: self.chars.next_if(|&(_, ch)| ch == '\n').is_some(),
            },
            '[' => OpenBracket {
                lf: self.chars.next_if(|&(_, ch)| ch == '\n').is_some(),
            },
            '(' => OpenParen {
                lf: self.chars.next_if(|&(_, ch)| ch == '\n').is_some(),
            },
            '}' => CloseBrace,
            ']' => CloseBracket,
            ')' => CloseParen,
            ch if is_delim(ch) => Punct,

            // appears only in the context
            '\0' => match self.chars.next() {
                Some((_, 'e')) => Expr {
                    lf: self.chars.next_if(|&(_, ch)| ch == '\n').is_some(),
                },
                Some((_, 'w')) => Where {
                    lf: self.chars.next_if(|&(_, ch)| ch == '\n').is_some(),
                },
                _ => Punct,
            },

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
        while self.chars.next().is_some() {}
        LineComment
    }

    fn block_comment(&mut self) -> TokenKind {
        self.chars.next();
        let mut depth = 1;
        while let Some((_, ch)) = self.chars.next() {
            match (ch, self.chars.peek()) {
                ('/', Some(&(_, '*'))) => {
                    self.chars.next();
                    depth += 1;
                }
                ('*', Some(&(_, '/'))) => {
                    self.chars.next();
                    depth -= 1;
                    if depth == 0 {
                        return BlockComment { open: false, depth };
                    }
                }
                _ => (),
            }
        }
        BlockComment { open: true, depth }
    }

    fn char_lit(&mut self) -> TokenKind {
        while let Some((_, ch)) = self.chars.next() {
            match ch {
                '\'' => return CharLit,
                '\\' => {
                    self.chars.next();
                }
                _ => (),
            }
        }
        CharLit
    }

    fn char_lit_or_lifetime(&mut self) -> TokenKind {
        self.chars.next();
        while let Some(&(_, ch)) = self.chars.peek() {
            match ch {
                '\'' => {
                    self.chars.next();
                    return CharLit;
                }
                ch if !is_delim(ch) => {
                    self.chars.next();
                }
                _ => return Lifetime,
            }
        }
        Lifetime
    }

    fn str_lit(&mut self) -> TokenKind {
        while let Some((_, ch)) = self.chars.next() {
            match ch {
                '"' => return StrLit { open: false },
                '\\' => {
                    self.chars.next();
                }
                _ => (),
            }
        }
        StrLit { open: true }
    }

    #[rustfmt::skip]
    fn raw_str_lit(&mut self) -> TokenKind {
        let mut n_hashes = 0;
        while let Some(&(_, '#')) = self.chars.peek() {
            self.chars.next();
            n_hashes += 1;
        }
        if let Some(&(_, '"')) = self.chars.peek() {
            self.chars.next();
        } else {
            return Punct;
        }
        while self.chars.any(|(_, ch)| ch == '"') {
            let mut close_hashes = 0;
            if close_hashes == n_hashes {
                return RawStrLit { open: false, n_hashes };
            }
            while let Some(&(_, '#')) = self.chars.peek() {
                self.chars.next();
                close_hashes += 1;
                if close_hashes == n_hashes {
                    return RawStrLit { open: false, n_hashes };
                }
            }
        }
        RawStrLit { open: true, n_hashes }
    }

    fn number_lit(&mut self) -> TokenKind {
        while let Some(&(_, '0'..='9' | '_')) = self.chars.peek() {
            self.chars.next();
        }
        if let Some(&(_, '.')) = self.chars.peek() {
            match self.chars.clone().nth(1) {
                Some((_, '0'..='9')) => {
                    self.chars.nth(1);
                    while let Some(&(_, '0'..='9' | '_')) = self.chars.peek() {
                        self.chars.next();
                    }
                }
                Some((_, '.')) => return NumberLit,
                Some((_, ch)) if !is_delim(ch) => return NumberLit,
                _ => {
                    self.chars.next();
                    return NumberLit;
                }
            }
        }
        if let Some(&(_, 'e' | 'E')) = self.chars.peek() {
            self.chars.next();
            self.chars.next_if(|&(_, ch)| ch == '+' || ch == '-');
            while let Some(&(_, '0'..='9' | '_')) = self.chars.peek() {
                self.chars.next();
            }
        }
        if let Some(&(idx, 'f' | 'i' | 'u')) = self.chars.peek() {
            self.chars.next();
            self.ident(idx);
        }
        NumberLit
    }

    fn n_ary_lit(&mut self, radix: u32) -> TokenKind {
        self.chars.next();
        while self
            .chars
            .next_if(|&(_, ch)| ch.is_digit(radix) || ch == '_')
            .is_some()
        {}
        if let Some(&(idx, 'f' | 'i' | 'u')) = self.chars.peek() {
            self.chars.next();
            self.ident(idx);
        }
        NumberLit
    }

    fn raw_ident(&mut self) -> TokenKind {
        self.chars.next();
        while self.chars.next_if(|&(_, ch)| !is_delim(ch)).is_some() {}
        Ident
    }

    fn upper_ident(&mut self) -> TokenKind {
        while self.chars.next_if(|&(_, ch)| !is_delim(ch)).is_some() {}
        UpperIdent
    }

    fn ident(&mut self, start: usize) -> TokenKind {
        while self.chars.next_if(|&(_, ch)| !is_delim(ch)).is_some() {}
        let end = self.chars.peek().map_or(self.text.len(), |&(idx, _)| idx);
        match &self.text[start..end] {
            "const" => Const,
            "fn" => Fn,
            "for" => For,
            "let" => Let,
            "mod" => Mod,
            "mut" => Mut,
            "static" => Static,
            "where" => Where { lf: false },
            "as" | "async" | "await" | "break" | "continue" | "crate" | "dyn" | "else" | "enum"
            | "extern" | "false" | "if" | "impl" | "in" | "loop" | "match" | "move" | "pub"
            | "ref" | "return" | "self" | "struct" | "super" | "trait" | "true" | "type"
            | "unsafe" | "use" | "while" => Keyword,
            "bool" | "char" | "f32" | "f64" | "i8" | "i16" | "i32" | "i64" | "i128" | "isize"
            | "str" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => PrimitiveType,
            _ => Ident,
        }
    }
}
