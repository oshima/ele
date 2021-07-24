use std::iter::{self, Chain, Iterator, Peekable, Repeat, Zip};
use std::str::{CharIndices, Chars};

use self::TokenKind::*;
use crate::canvas::Term;
use crate::face::{Bg, Fg};
use crate::row::Row;
use crate::syntax::Syntax;

pub struct Ruby;

impl Syntax for Ruby {
    fn name(&self) -> &'static str {
        "Ruby"
    }

    fn fg_color(&self, term: Term) -> &'static [u8] {
        match term {
            Term::TrueColor => fg_color!(255, 255, 255),
            Term::Color256 => fg_color256!(231),
            Term::Color16 => fg_color16!(white),
        }
    }

    fn bg_color(&self, term: Term) -> &'static [u8] {
        match term {
            Term::TrueColor => bg_color!(112, 21, 22),
            Term::Color256 => bg_color256!(88),
            Term::Color16 => bg_color16!(red),
        }
    }

    fn indent_unit(&self) -> Option<&'static str> {
        None
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

impl Ruby {
    #![allow(clippy::single_match)]
    fn update_row(&self, row: &mut Row, context_v: &mut Vec<TokenKind>, context_s: &mut String) {
        let mut tokens = Tokens::from(&row.string, row.context.as_deref().unwrap()).peekable();
        let mut prev_token: Option<Token> = None;

        row.faces.clear();
        row.faces
            .resize(row.string.len(), (Fg::Default, Bg::Default));
        row.trailing_bg = Bg::Default;
        row.indent_level = 0;

        while let Some(token) = tokens.next() {
            // Highlight
            let fg = match token.kind {
                Comment => Fg::Comment,
                Def | Keyword | KeywordWithExpr => Fg::Keyword,
                Key => Fg::Macro,
                RegexpLit { .. } | StrLit { .. } => Fg::String,
                SymbolLit { .. } => Fg::Macro,
                UpperIdent => Fg::Type,
                Ident => match prev_token.map(|t| t.kind) {
                    Some(Def) => Fg::Function,
                    _ => Fg::Default,
                },
                _ => Fg::Default,
            };

            for i in token.start..token.end {
                row.faces[i].0 = fg;
            }

            // Derive the context of the next row
            match token.kind {
                RegexpLit { open: true, .. }
                | StrLit { open: true, .. }
                | SymbolLit { open: true, .. } => {
                    context_v.push(token.kind);
                }
                _ => (),
            }

            prev_token = Some(token);
        }

        self.convert_context(context_v, context_s);
    }

    #[rustfmt::skip]
    fn convert_context(&self, slice: &[TokenKind], string: &mut String) {
        for token_kind in slice {
            match *token_kind {
                RegexpLit { open: true, delim } => {
                    match delim {
                        Some('/') => {
                            string.push('/');
                        }
                        Some(ch) => {
                            string.push_str("%r");
                            string.push(ch);
                        }
                        _ => (),
                    }
                },
                StrLit { open: true, delim } => {
                    match delim {
                        Some(ch @ '\'' | ch @ '"' | ch @ '`') => {
                            string.push(ch);
                        }
                        Some(ch) => {
                            string.push('%');
                            string.push(ch);
                        }
                        _ => (),
                    }
                },
                SymbolLit { open: true, delim } => {
                    match delim {
                        Some(ch @ '\'' | ch @ '"') => {
                            string.push(':');
                            string.push(ch);
                        }
                        Some(ch) => {
                            string.push_str("%s");
                            string.push(ch);
                        }
                        _ => (),
                    }
                },
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
    Bar,
    Comma,
    Comment,
    Def,
    Ident,
    Key,
    Keyword,
    KeywordWithExpr,
    OpenBrace,
    OpenBracket,
    OpenParen,
    Op,
    Punct,
    RegexpLit { open: bool, delim: Option<char> },
    Semi,
    StrLit { open: bool, delim: Option<char> },
    SymbolLit { open: bool, delim: Option<char> },
    UpperIdent,
}

struct Tokens<'a> {
    text: &'a str,
    chars: Peekable<Chain<Zip<Repeat<usize>, Chars<'a>>, CharIndices<'a>>>,
    prev: Option<Token>,
}

impl<'a> Tokens<'a> {
    fn from(text: &'a str, context: &'a str) -> Self {
        Self {
            text,
            chars: iter::repeat(0)
                .zip(context.chars())
                .chain(text.char_indices())
                .peekable(),
            prev: None,
        }
    }
}

fn is_delim(ch: char) -> bool {
    match ch {
        '_' | '!' | '?' => false,
        ch if ch.is_ascii_whitespace() => true,
        ch if ch.is_ascii_punctuation() => true,
        _ => false,
    }
}

impl<'a> Iterator for Tokens<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        let (start, ch) = self.chars.find(|&(_, ch)| !ch.is_ascii_whitespace())?;

        let kind = match ch {
            // comment
            '#' => self.comment(),

            // string
            '\'' | '"' | '`' => self.str_lit(Some(ch)),

            // symbol
            ':' => match self.chars.next_if(|&(_, ch)| ch == ':') {
                Some(_) => Punct,
                _ => match self.chars.peek() {
                    None => Op,
                    Some(&(_, ch)) if ch.is_ascii_whitespace() => Op,
                    Some(&(_, ch @ '\'' | ch @ '"')) => {
                        self.chars.next();
                        self.symbol_lit(Some(ch))
                    }
                    _ => self.symbol_lit(None),
                },
            },

            // regexp
            '/' => match self.prev {
                Some(prev) => match prev.kind {
                    Bar | Comma | KeywordWithExpr | OpenBrace | OpenBracket | OpenParen | Op
                    | Semi => self.regexp_lit(Some('/')),
                    Ident => match start == prev.end {
                        true => Op,
                        false => match self.chars.peek() {
                            None => Op,
                            Some(&(_, ch)) if ch.is_ascii_whitespace() => Op,
                            _ => self.regexp_lit(Some('/')),
                        },
                    },
                    _ => Op,
                },
                None => self.regexp_lit(Some('/')),
            },

            // percent literal
            '%' => match self.prev {
                Some(prev) => match prev.kind {
                    Bar | Comma | KeywordWithExpr | OpenBrace | OpenBracket | OpenParen | Op
                    | Semi => self.percent_lit(),
                    Ident => match start == prev.end {
                        true => Op,
                        false => match self.chars.peek() {
                            None => Op,
                            Some(&(_, ch)) if ch.is_ascii_whitespace() => Op,
                            _ => self.percent_lit(),
                        },
                    },
                    _ => Op,
                },
                None => self.percent_lit(),
            },

            // punctuation
            ',' => Comma,
            '{' => OpenBrace,
            '[' => OpenBracket,
            '(' => OpenParen,
            ';' => Semi,
            '|' => match self.chars.next_if(|&(_, ch)| ch == '|') {
                Some(_) => Op,
                _ => Bar,
            },

            // operator
            '!' | '&' | '*' | '+' | '-' | '<' | '=' | '>' | '?' | '^' => Op,
            '.' => match self.chars.next_if(|&(_, ch)| ch == '.') {
                Some(_) => {
                    self.chars.next_if(|&(_, ch)| ch == '.');
                    Op
                }
                _ => Punct,
            },
            ch if is_delim(ch) => Punct,

            // identifier or keyword
            ch if ch.is_ascii_uppercase() => self.upper_ident(),
            _ => self.ident_or_keyword(start),
        };

        let end = self.chars.peek().map_or(self.text.len(), |&(idx, _)| idx);
        let token = Token { kind, start, end };
        self.prev.replace(token);

        Some(token)
    }
}

impl<'a> Tokens<'a> {
    fn consume_content(&mut self, delim: char) -> bool {
        let mut depth = 1;
        let close_delim = match delim {
            '(' => ')',
            '<' => '>',
            '[' => ']',
            '{' => '}',
            ch => ch,
        };
        while let Some((_, ch)) = self.chars.next() {
            match ch {
                ch if ch == close_delim => {
                    depth -= 1;
                    if depth == 0 {
                        return false;
                    }
                }
                ch if ch == delim => {
                    depth += 1;
                }
                '\\' => {
                    self.chars.next();
                }
                _ => (),
            }
        }
        true
    }

    fn comment(&mut self) -> TokenKind {
        while self.chars.next().is_some() {}
        Comment
    }

    fn str_lit(&mut self, delim: Option<char>) -> TokenKind {
        if let Some(ch) = delim {
            let open = self.consume_content(ch);
            StrLit { open, delim }
        } else {
            StrLit { open: false, delim }
        }
    }

    #[rustfmt::skip]
    fn symbol_lit(&mut self, delim: Option<char>) -> TokenKind {
        if let Some(ch) = delim {
            let open = self.consume_content(ch);
            SymbolLit { open, delim }
        } else {
            // TODO
            // MEMO: 別メソッド gprimary_symbol_lit にした方がよさそう
            while self.chars.next_if(|&(_, ch)| !is_delim(ch)).is_some() {}
            SymbolLit { open: false, delim }
        }
    }

    fn regexp_lit(&mut self, delim: Option<char>) -> TokenKind {
        if let Some(ch) = delim {
            let open = self.consume_content(ch);
            while self
                .chars
                .next_if(|&(_, ch)| matches!(ch, 'i' | 'm' | 'o' | 'x'))
                .is_some()
            {}
            RegexpLit { open, delim }
        } else {
            RegexpLit { open: false, delim }
        }
    }

    fn percent_lit(&mut self) -> TokenKind {
        match self.chars.peek() {
            Some(&(_, 'Q'| 'W' | 'q' | 'w' | 'x')) => {
                self.chars.next();
                match self.chars.next_if(|&(_, ch)| ch.is_ascii_punctuation()) {
                    Some((_, ch)) => self.str_lit(Some(ch)),
                    _ => self.str_lit(None),
                }
            }
            Some(&(_, 'I' | 'i' | 's')) => {
                self.chars.next();
                match self.chars.next_if(|&(_, ch)| ch.is_ascii_punctuation()) {
                    Some((_, ch)) => self.symbol_lit(Some(ch)),
                    _ => SymbolLit { open: false, delim: None },
                }
            }
            Some(&(_, 'r')) => {
                self.chars.next();
                match self.chars.next_if(|&(_, ch)| ch.is_ascii_punctuation()) {
                    Some((_, ch)) => self.regexp_lit(Some(ch)),
                    _ => self.regexp_lit(None),
                }
            }
            Some(&(_, ch)) if ch.is_ascii_punctuation() => {
                self.chars.next();
                self.str_lit(Some(ch))
            }
            _ => Op,
        }
    }

    fn upper_ident(&mut self) -> TokenKind {
        while self.chars.next_if(|&(_, ch)| !is_delim(ch)).is_some() {}
        UpperIdent
    }

    fn ident_or_keyword(&mut self, start: usize) -> TokenKind {
        while self.chars.next_if(|&(_, ch)| !is_delim(ch)).is_some() {}
        if self.chars.next_if(|&(_, ch)| ch == ':').is_some() {
            return Key;
        }
        let end = self.chars.peek().map_or(self.text.len(), |&(idx, _)| idx);
        match &self.text[start..end] {
            "def" => Def,
            "alias" | "class" | "defined?" | "else" | "ensure" | "false" | "for" | "end"
            | "in" | "module" | "next" | "nil" | "redo" | "retry" | "self" | "super"
            | "then" | "true" | "undef" | "yield" => Keyword,
            "and" | "begin" | "break" | "case" | "do" | "elsif" | "fail" | "if" | "not"
            | "or" | "rescue" | "return" | "unless" | "until" | "when" | "while" => {
                KeywordWithExpr
            }
            _ => Ident,
        }
    }
}
