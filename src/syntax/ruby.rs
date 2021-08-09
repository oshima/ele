use std::iter::{self, Chain, Iterator, Peekable, Repeat, Zip};
use std::str::{CharIndices, Chars};

use self::ExpansionKind::*;
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

        row.faces.clear();
        row.faces
            .resize(row.string.len(), (Fg::Default, Bg::Default));
        row.trailing_bg = Bg::Default;
        row.indent_level = 0;

        while let Some(token) = tokens.next() {
            // Highlight
            let fg = match token.kind {
                BuiltinMethod { takes_arg: false } => Fg::Macro,
                CloseExpansion { .. } | OpenExpansion { .. } => Fg::Variable,
                Comment => Fg::Comment,
                Def | Do | Keyword { .. } => Fg::Keyword,
                Heredoc { .. } | HeredocLabel { .. } => Fg::String,
                Key => Fg::Macro,
                Method => Fg::Function,
                MethodOwner => Fg::Variable,
                PureSymbolLit | SymbolLit { .. } => Fg::Macro,
                RegexpLit { .. } | StrLit { .. } => Fg::String,
                UpperIdent => Fg::Type,
                Variable => Fg::Variable,
                BuiltinMethod { takes_arg: true } => match tokens.peek().map(|t| &t.kind) {
                    Some(
                        CloseBrace
                        | CloseExpansion { .. }
                        | Comma
                        | Comment
                        | Def
                        | Dot
                        | Keyword { .. }
                        | Op
                        | Punct
                        | Semi,
                    )
                    | None => Fg::Default,
                    _ => Fg::Macro,
                },
                Ident => match tokens.peek().map(|t| &t.kind) {
                    Some(
                        CloseBrace
                        | CloseExpansion { .. }
                        | Comma
                        | Comment
                        | Def
                        | Dot
                        | Keyword { .. }
                        | Op
                        | OpenBracket
                        | Punct
                        | Semi,
                    )
                    | None => Fg::Default,
                    _ => Fg::Function,
                },
                _ => Fg::Default,
            };

            for i in token.start..token.end {
                row.faces[i].0 = fg;
            }

            // Derive the context of the next row
            match token.kind {
                RegexpLit { depth: 0, .. } => match context_v[..] {
                    [.., RegexpLit { .. }] => {
                        context_v.pop();
                    }
                    _ => (),
                },
                RegexpLit { .. } => match context_v[..] {
                    [.., RegexpLit { .. }] => {
                        context_v.pop();
                        context_v.push(token.kind);
                    }
                    _ => {
                        context_v.push(token.kind);
                    }
                },
                StrLit { depth: 0, .. } => match context_v[..] {
                    [.., StrLit { .. }] => {
                        context_v.pop();
                    }
                    _ => (),
                },
                StrLit { .. } => match context_v[..] {
                    [.., StrLit { .. }] => {
                        context_v.pop();
                        context_v.push(token.kind);
                    }
                    _ => {
                        context_v.push(token.kind);
                    }
                },
                SymbolLit { depth: 0, .. } => match context_v[..] {
                    [.., SymbolLit { .. }] => {
                        context_v.pop();
                    }
                    _ => (),
                },
                SymbolLit { .. } => match context_v[..] {
                    [.., SymbolLit { .. }] => {
                        context_v.pop();
                        context_v.push(token.kind);
                    }
                    _ => {
                        context_v.push(token.kind);
                    }
                },
                Heredoc { .. } => match context_v[..] {
                    [.., Heredoc { .. }] => {
                        context_v.pop();
                        context_v.push(token.kind);
                    }
                    _ => {
                        context_v.push(token.kind);
                    }
                }
                HeredocLabel { label: Some(_), .. } => {
                    context_v.push(token.kind);
                }
                OpenBrace | OpenExpansion { .. } => {
                    context_v.push(token.kind);
                }
                CloseBrace => match context_v[..] {
                    [.., OpenBrace] => {
                        context_v.pop();
                    }
                    _ => (),
                },
                CloseExpansion { .. } => match context_v[..] {
                    [.., OpenExpansion { .. }] => {
                        context_v.pop();
                    }
                    _ => (),
                },
                _ => (),
            }
        }

        self.convert_context(context_v, context_s);
    }

    #[rustfmt::skip]
    fn convert_context(&self, slice: &[TokenKind], string: &mut String) {
        for token_kind in slice {
            match token_kind {
                RegexpLit { delim, expand, depth } => match (delim, expand) {
                    ('/', true) => {
                        string.push(*delim);
                    }
                    _ => {
                        string.push_str("%r");
                        for _ in 0..*depth {
                            string.push(*delim);
                        }
                    }
                },
                StrLit { delim, expand, depth } => match (delim, expand) {
                    ('\'', false) | ('"' | '`', true) => {
                        string.push(*delim);
                    }
                    _ => {
                        string.push('%');
                        string.push(if *expand { 'Q' } else { 'q' });
                        for _ in 0..*depth {
                            string.push(*delim);
                        }
                    }
                },
                SymbolLit { delim, expand, depth } => match (delim, expand) {
                    ('\'', false) | ('"', true) => {
                        string.push(':');
                        string.push(*delim);
                    }
                    _ => {
                        string.push('%');
                        string.push(if *expand { 'I' } else { 'i' });
                        for _ in 0..*depth {
                            string.push(*delim);
                        }
                    }
                },
                HeredocLabel { label: Some(label), expand } => {
                    string.push('\0');
                    string.push(if *expand { '"' } else { '\'' });
                    string.push_str(label);
                    string.push(if *expand { '"' } else { '\'' });
                }
                Heredoc { label, expand, trailing_context, open } => {
                    if *open {
                        string.push('\0');
                        string.push(if *expand { '"' } else { '\'' });
                        string.push_str(label);
                        string.push(if *expand { '"' } else { '\'' });
                    }
                    string.push_str(trailing_context);
                }
                OpenBrace => {
                    string.push('{');
                }
                OpenExpansion { .. } => {
                    string.push_str("#{");
                }
                _ => (),
            }
        }
    }
}

#[derive(Clone)]
struct Token {
    kind: TokenKind,
    start: usize,
    end: usize,
}

#[derive(Clone)]
#[rustfmt::skip]
enum TokenKind {
    BuiltinMethod { takes_arg: bool },
    CloseBrace,
    CloseExpansion { kind: ExpansionKind },
    ColonColon,
    Comma,
    Comment,
    Def,
    Do,
    Dot,
    Heredoc { label: String, expand: bool, trailing_context: String, open: bool },
    HeredocLabel { label: Option<String>, expand: bool },
    Ident,
    Key,
    Keyword { takes_expr: bool },
    Method,
    MethodOwner,
    OpenBrace,
    OpenBracket,
    OpenExpansion { kind: ExpansionKind },
    OpenParen,
    Op,
    Punct,
    PureSymbolLit,
    RegexpLit { delim: char, expand: bool, depth: usize },
    Semi,
    StrLit { delim: char, expand: bool, depth: usize },
    SymbolLit { delim: char, expand: bool, depth: usize },
    UpperIdent,
    Variable,
}

#[derive(Clone)]
#[rustfmt::skip]
enum ExpansionKind {
    InHeredoc { label: String, trailing_context: String },
    InRegexp { delim: char, depth: usize },
    InStr { delim: char, depth: usize },
    InSymbol { delim: char, depth: usize },
}

struct Tokens<'a> {
    text: &'a str,
    chars: Peekable<Chain<Zip<Repeat<usize>, Chars<'a>>, CharIndices<'a>>>,
    prev: Option<Token>,
    braces: Vec<TokenKind>,
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
            braces: Vec::new(),
        }
    }
}

fn is_delim(ch: char) -> bool {
    ch.is_ascii_whitespace() || ch != '_' && ch.is_ascii_punctuation()
}

impl<'a> Iterator for Tokens<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(CloseExpansion { kind }) = self.prev.clone().map(|t| t.kind) {
            let start = self.chars.peek().map_or(self.text.len(), |&(idx, _)| idx);
            #[rustfmt::skip]
            let kind = match kind {
                InHeredoc { label, trailing_context } => self.heredoc_temp(label, trailing_context),
                InRegexp { delim, depth } => self.regexp_lit(delim, true, depth),
                InStr { delim, depth } => self.str_lit(delim, true, depth),
                InSymbol { delim, depth } => self.symbol_lit(delim, true, depth),
            };
            let end = self.chars.peek().map_or(self.text.len(), |&(idx, _)| idx);

            let token = Token { kind, start, end };
            self.prev.replace(token.clone());

            return Some(token);
        }

        let (start, ch) = self.chars.find(|&(_, ch)| !ch.is_ascii_whitespace())?;

        let kind = match ch {
            // comment or expression expansion
            #[rustfmt::skip]
            '#' => match self.prev.as_ref().map(|t| &t.kind) {
                Some(Heredoc { label, trailing_context, open: true, .. }) => {
                    self.chars.next();
                    OpenExpansion { kind: InHeredoc { label: label.clone(), trailing_context: trailing_context.clone() } }
                }
                Some(RegexpLit { delim, depth, .. }) if *depth > 0 => {
                    self.chars.next();
                    OpenExpansion { kind: InRegexp { delim: *delim, depth: *depth } }
                }
                Some(StrLit { delim, depth, .. }) if *depth > 0 => {
                    self.chars.next();
                    OpenExpansion { kind: InStr { delim: *delim, depth: *depth } }
                }
                Some(SymbolLit { delim, depth, .. }) if *depth > 0 => {
                    self.chars.next();
                    OpenExpansion { kind: InSymbol { delim: *delim, depth: *depth } }
                }
                _ => self.comment(),
            },

            // string
            '\'' => self.str_lit(ch, false, 1),
            '"' | '`' => self.str_lit(ch, true, 1),

            // symbol
            ':' => match self.chars.peek() {
                Some(&(_, ' ' | '\t')) | None => Op,
                Some(&(_, ':')) => {
                    self.chars.next();
                    ColonColon
                }
                Some(&(_, '\'')) => {
                    self.chars.next();
                    self.symbol_lit('\'', false, 1)
                }
                Some(&(_, '"')) => {
                    self.chars.next();
                    self.symbol_lit('"', true, 1)
                }
                Some(&(_, ch)) => self.pure_symbol_lit(ch),
            },

            // regexp
            '/' => match self.prev.as_ref().map(|t| &t.kind) {
                Some(
                    Comma
                    | Key
                    | Keyword { takes_expr: true }
                    | Op
                    | OpenBrace
                    | OpenExpansion { .. }
                    | OpenBracket
                    | OpenParen
                    | Semi,
                ) => self.regexp_lit(ch, true, 1),
                Some(Def | Dot) => self.method(ch),
                Some(BuiltinMethod { takes_arg: true } | Ident) => {
                    match self.prev.as_ref().filter(|t| t.end == start) {
                        Some(_) => Op,
                        None => match self.chars.peek() {
                            Some(&(_, ' ' | '\t')) | None => Op,
                            _ => self.regexp_lit(ch, true, 1),
                        },
                    }
                }
                Some(_) => Op,
                None => self.regexp_lit(ch, true, 1),
            },

            // percent literal
            '%' => match self.prev.as_ref().map(|t| &t.kind) {
                Some(
                    Comma
                    | Key
                    | Keyword { takes_expr: true }
                    | Op
                    | OpenBrace
                    | OpenExpansion { .. }
                    | OpenBracket
                    | OpenParen
                    | Semi,
                ) => self.percent_lit(),
                Some(Def | Dot) => self.method(ch),
                Some(BuiltinMethod { takes_arg: true } | Ident) => {
                    match self.prev.as_ref().filter(|t| t.end == start) {
                        Some(_) => Op,
                        None => self.percent_lit(),
                    }
                }
                Some(_) => Op,
                None => self.percent_lit(),
            },

            // operator
            '?' => Op,
            '<' => match self.prev.as_ref().map(|t| &t.kind) {
                Some(Def | Dot) => self.method(ch),
                _ => self.heredoc_label(),
            }
            '!' | '&' | '*' | '+' | '-' | '=' | '>' | '^' | '|' | '~' => {
                match self.prev.as_ref().map(|t| &t.kind) {
                    Some(Def | Dot) => self.method(ch),
                    _ => Op,
                }
            }
            '.' => match self.chars.next_if(|&(_, ch)| ch == '.') {
                Some(_) => {
                    self.chars.next_if(|&(_, ch)| ch == '.');
                    Op
                }
                _ => Dot,
            },

            // variable
            '@' => self.variable(),

            // punctuation
            ',' => Comma,
            ';' => Semi,
            '{' => OpenBrace,
            '[' => match self.prev.as_ref().map(|t| &t.kind) {
                Some(Def | Dot) => self.method(ch),
                _ => OpenBracket,
            },
            '(' => OpenParen,
            '}' => match self.braces.last() {
                Some(OpenExpansion { kind }) => CloseExpansion { kind: kind.clone() },
                _ => CloseBrace,
            },
            ch if is_delim(ch) => Punct,

            // heredoc
            '\0' => self.heredoc(),

            // identifier or keyword
            ch if ch.is_ascii_uppercase() => match self.prev.as_ref().map(|t| &t.kind) {
                Some(Def) => self.method_owner_or_method(),
                Some(Dot) => self.method(ch),
                _ => self.upper_ident(),
            },
            _ => match self.prev.as_ref().map(|t| &t.kind) {
                Some(Def) => self.method_owner_or_method(),
                Some(Dot) => self.method(ch),
                _ => self.ident_or_keyword(start),
            },
        };

        let end = self.chars.peek().map_or(self.text.len(), |&(idx, _)| idx);

        let token = Token { kind, start, end };
        self.prev.replace(token.clone());
        match token.kind {
            OpenBrace | OpenExpansion { .. } => {
                self.braces.push(token.kind.clone());
            }
            CloseBrace | CloseExpansion { .. } => {
                self.braces.pop();
            }
            _ => (),
        };

        Some(token)
    }
}

impl<'a> Tokens<'a> {
    fn consume_content(&mut self, delim: char, expand: bool, depth: usize) -> usize {
        let mut depth = depth;
        let close_delim = match delim {
            '(' => ')',
            '<' => '>',
            '[' => ']',
            '{' => '}',
            _ => delim,
        };
        while let Some(&(_, ch)) = self.chars.peek() {
            match ch {
                ch if ch == close_delim => {
                    self.chars.next();
                    depth -= 1;
                    if depth == 0 {
                        return depth;
                    }
                }
                ch if ch == delim => {
                    self.chars.next();
                    depth += 1;
                }
                '\\' => {
                    self.chars.next();
                    self.chars.next();
                }
                '#' => {
                    if expand && matches!(self.chars.clone().nth(1), Some((_, '{'))) {
                        return depth;
                    } else {
                        self.chars.next();
                    }
                }
                _ => {
                    self.chars.next();
                }
            }
        }
        depth
    }

    fn comment(&mut self) -> TokenKind {
        while self.chars.next().is_some() {}
        Comment
    }

    #[rustfmt::skip]
    fn str_lit(&mut self, delim: char, expand: bool, depth: usize) -> TokenKind {
        let depth = self.consume_content(delim, expand, depth);
        StrLit { delim, expand, depth }
    }

    #[rustfmt::skip]
    fn symbol_lit(&mut self, delim: char, expand: bool, depth: usize) -> TokenKind {
        let depth = self.consume_content(delim, expand, depth);
        SymbolLit { delim, expand, depth }
    }

    fn pure_symbol_lit(&mut self, peeked: char) -> TokenKind {
        let valid = match peeked {
            '!' | '%' | '&' | '*' | '+' | '-' | '/' | '<' | '>' | '^' | '|' | '~' => true,
            '=' => matches!(self.chars.clone().nth(1), Some((_, '=' | '~'))),
            '[' => matches!(self.chars.clone().nth(1), Some((_, ']'))),
            ch if !is_delim(ch) && !ch.is_ascii_digit() => true,
            _ => false,
        };
        if valid {
            self.chars.next();
            self.method(peeked);
            PureSymbolLit
        } else {
            Op
        }
    }

    #[rustfmt::skip]
    fn regexp_lit(&mut self, delim: char, expand: bool, depth: usize) -> TokenKind {
        let depth = self.consume_content(delim, expand, depth);
        while let Some(&(_, 'i' | 'm' | 'o' | 'x')) = self.chars.peek() {
            self.chars.next();
        }
        RegexpLit { delim, expand, depth }
    }

    #[rustfmt::skip]
    fn percent_lit(&mut self) -> TokenKind {
        match self.chars.peek() {
            Some(&(_, ch)) if ch.is_ascii_punctuation() => {
                self.chars.next();
                self.str_lit(ch, true, 1)
            }
            Some(&(_, 'q' | 'w')) => match self.chars.clone().nth(1) {
                Some((_, ch)) if ch.is_ascii_punctuation() => {
                    self.chars.next();
                    self.chars.next();
                    self.str_lit(ch, false, 1)
                }
                _ => Op,
            }
            Some(&(_, 'Q' | 'W' | 'x')) => match self.chars.clone().nth(1) {
                Some((_, ch)) if ch.is_ascii_punctuation() => {
                    self.chars.next();
                    self.chars.next();
                    self.str_lit(ch, true, 1)
                }
                _ => Op,
            }
            Some(&(_, 'i' | 's')) => match self.chars.clone().nth(1) {
                Some((_, ch)) if ch.is_ascii_punctuation() => {
                    self.chars.next();
                    self.chars.next();
                    self.symbol_lit(ch, false, 1)
                }
                _ => Op,
            }
            Some(&(_, 'I')) => match self.chars.clone().nth(1) {
                Some((_, ch)) if ch.is_ascii_punctuation() => {
                    self.chars.next();
                    self.chars.next();
                    self.symbol_lit(ch, true, 1)
                }
                _ => Op,
            }
            Some(&(_, 'r')) => match self.chars.clone().nth(1) {
                Some((_, ch)) if ch.is_ascii_punctuation() => {
                    self.chars.next();
                    self.chars.next();
                    self.regexp_lit(ch, true, 1)
                }
                _ => Op,
            }
            _ => Op,
        }
    }

    fn heredoc_label(&mut self) -> TokenKind {
        match self.chars.peek() {
            Some(&(_, '<')) => match self.chars.clone().nth(1) {
                Some((start, ch)) if !is_delim(ch) => {
                    self.chars.nth(1);
                    while self.chars.next_if(|&(_, ch)| !is_delim(ch)).is_some() {}
                    let end = self.chars.peek().map_or(self.text.len(), |&(idx, _)| idx);
                    let label = Some(self.text[start..end].to_string());
                    HeredocLabel { label, expand: true }
                }
                Some((start, d @ '\'' | d @ '"' | d @ '`')) => {
                    self.chars.nth(1);
                    let label = self.chars
                        .find(|&(_, ch)| ch == d)
                        .map(|(end, _)| self.text[(start + 1)..end].to_string());
                    HeredocLabel { label, expand: d != '\'' }
                }
                Some((_, '-' | '~')) => match self.chars.clone().nth(2) {
                    Some((start, ch)) if !is_delim(ch) => {
                        self.chars.nth(2);
                        while self.chars.next_if(|&(_, ch)| !is_delim(ch)).is_some() {}
                        let end = self.chars.peek().map_or(self.text.len(), |&(idx, _)| idx);
                        let label = Some(self.text[start..end].to_string());
                        HeredocLabel { label, expand: true }
                    }
                    Some((start, d @ '\'' | d @ '"' | d @ '`')) => {
                        self.chars.nth(2);
                        let label = self.chars
                            .find(|&(_, ch)| ch == d)
                            .map(|(end, _)| self.text[(start + 1)..end].to_string());
                        HeredocLabel { label, expand: d != '\'' }
                    }
                    _ => Op,
                }
                _ => Op,
            }
            _ => Op,
        }
    }

    fn heredoc(&mut self) -> TokenKind {
        let delim = self.chars.next().map(|(_, ch)| ch).unwrap();
        let label: String = self.chars
            .by_ref()
            .map(|(_, ch)| ch)
            .take_while(|&ch| ch != delim)
            .collect();
        let expand = delim == '"';
        let mut trailing_context = String::new();
        while let Some(&(_, ch)) = self.chars.peek() {
            match (ch, self.chars.clone().nth(1)) {
                ('#', Some((_, '{'))) => {
                    return Heredoc { label, expand, trailing_context, open: true };
                },
                (ch, Some((idx, _))) if idx == 0 => {
                    trailing_context.push(ch);
                }
                (ch, None) => {
                    if self.text.is_empty() {
                        trailing_context.push(ch);
                    }
                }
                _ => break,
            }
            self.chars.next();
        }
        while let Some(&(_, ch)) = self.chars.peek() {
            if expand && ch == '#' && matches!(self.chars.clone().nth(1), Some((_, '{'))) {
                return Heredoc { label, expand, trailing_context, open: true };
            }
            self.chars.next();
        }
        let open = !self.text.contains(&label) || self.text.trim() != label.trim();
        Heredoc { label, expand, trailing_context, open }
    }

    fn heredoc_temp(&mut self, label: String, trailing_context: String) -> TokenKind {
        while let Some(&(_, ch)) = self.chars.peek() {
            if ch == '#' && matches!(self.chars.clone().nth(1), Some((_, '{'))) {
                return Heredoc { label, expand: true, trailing_context, open: true };
            }
            self.chars.next();
        }
        Heredoc { label, expand: true, trailing_context, open: true }
    }

    fn variable(&mut self) -> TokenKind {
        self.chars.next_if(|&(_, ch)| ch == '@');
        while self.chars.next_if(|&(_, ch)| !is_delim(ch)).is_some() {}
        Variable
    }

    fn method_owner_or_method(&mut self) -> TokenKind {
        while self.chars.next_if(|&(_, ch)| !is_delim(ch)).is_some() {}
        match self.chars.peek() {
            Some(&(_, '!' | '?' | '=')) => {
                self.chars.next();
                Method
            }
            Some(&(_, '.')) => MethodOwner,
            _ => Method,
        }
    }

    fn method(&mut self, ch: char) -> TokenKind {
        match ch {
            '|' | '^' | '&' | '/' | '%' | '~' | '`' => (),
            '+' | '-' => {
                self.chars.next_if(|&(_, ch)| ch == '@');
            }
            '*' => {
                self.chars.next_if(|&(_, ch)| ch == '*');
            }
            '<' => match self.chars.peek() {
                Some(&(_, '<')) => {
                    self.chars.next();
                }
                Some(&(_, '=')) => {
                    self.chars.next();
                    self.chars.next_if(|&(_, ch)| ch == '>');
                }
                _ => (),
            },
            '>' => {
                self.chars.next_if(|&(_, ch)| ch == '>' || ch == '=');
            }
            '!' => {
                self.chars.next_if(|&(_, ch)| ch == '=' || ch == '~');
            }
            '=' => match self.chars.peek() {
                Some(&(_, '=')) => {
                    self.chars.next();
                    self.chars.next_if(|&(_, ch)| ch == '=');
                }
                Some(&(_, '~')) => {
                    self.chars.next();
                }
                _ => return Op,
            },
            '[' => match self.chars.peek() {
                Some(&(_, ']')) => {
                    self.chars.next();
                    self.chars.next_if(|&(_, ch)| ch == '=');
                }
                _ => return OpenBracket,
            },
            _ => {
                while self.chars.next_if(|&(_, ch)| !is_delim(ch)).is_some() {}
                self.chars.next_if(|&(_, ch)| ch == '!' || ch == '?' || ch == '=');
            }
        }
        Method
    }

    fn upper_ident(&mut self) -> TokenKind {
        while self.chars.next_if(|&(_, ch)| !is_delim(ch)).is_some() {}
        self.chars.next_if(|&(_, ch)| ch == '!' || ch == '?');
        if let Some(&(_, ':')) = self.chars.peek() {
            if !matches!(self.chars.clone().nth(1), Some((_, ':'))) {
                self.chars.next();
                return Key;
            }
        }
        UpperIdent
    }

    fn ident_or_keyword(&mut self, start: usize) -> TokenKind {
        while self.chars.next_if(|&(_, ch)| !is_delim(ch)).is_some() {}
        self.chars.next_if(|&(_, ch)| ch == '!' || ch == '?');
        if let Some(&(_, ':')) = self.chars.peek() {
            if !matches!(self.chars.clone().nth(1), Some((_, ':'))) {
                self.chars.next();
                return Key;
            }
        }
        let end = self.chars.peek().map_or(self.text.len(), |&(idx, _)| idx);
        match &self.text[start..end] {
            "def" => Def,
            "do" => Do,
            "alias" | "class" | "defined?" | "else" | "ensure" | "false" | "for" | "end" | "in"
            | "module" | "next" | "nil" | "redo" | "rescue" | "retry" | "self" | "super"
            | "then" | "true" | "undef" | "yield" => Keyword { takes_expr: false },
            "and" | "begin" | "break" | "case" | "elsif" | "if" | "not" | "or" | "return"
            | "unless" | "until" | "when" | "while" => Keyword { takes_expr: true },
            "__callee__" | "__dir__" | "__method__" | "abort" | "binding" | "block_given?"
            | "caller" | "exit" | "exit!" | "fail" | "fork" | "global_variables"
            | "local_variables" | "private" | "protected" | "public" | "raise" | "rand"
            | "readline" | "readlines" | "sleep" | "srand" => BuiltinMethod { takes_arg: false },
            "alias_method"
            | "at_exit"
            | "attr"
            | "attr_accessor"
            | "attr_reader"
            | "attr_writer"
            | "autoload"
            | "autoload?"
            | "callcc"
            | "catch"
            | "define_method"
            | "eval"
            | "exec"
            | "extend"
            | "format"
            | "include"
            | "lambda"
            | "load"
            | "loop"
            | "module_function"
            | "open"
            | "p"
            | "prepend"
            | "print"
            | "printf"
            | "private_class_method"
            | "private_constant"
            | "proc"
            | "public_class_method"
            | "public_constant"
            | "putc"
            | "puts"
            | "refine"
            | "require"
            | "require_relative"
            | "spawn"
            | "sprintf"
            | "syscall"
            | "system"
            | "throw"
            | "trace_var"
            | "trap"
            | "untrace_var"
            | "using"
            | "warn" => BuiltinMethod { takes_arg: true },
            _ => Ident,
        }
    }
}
