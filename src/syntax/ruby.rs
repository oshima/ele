use std::iter::{self, Chain, Iterator, Peekable, Repeat, Zip};
use std::str::CharIndices;

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
    fn update_row<'a>(
        &self,
        row: &'a mut Row,
        context_v: &mut Vec<TokenKind<'a>>,
        context_s: &mut String,
    ) {
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
                BuiltinMethod { takes_arg: true } => match tokens.peek().map(|t| t.kind) {
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
                Ident => match tokens.peek().map(|t| t.kind) {
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
                },
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
            match *token_kind {
                RegexpLit { delim, expand, depth } => match (delim, expand) {
                    ('/', true) => {
                        string.push(delim);
                    }
                    _ => {
                        string.push_str("%r");
                        for _ in 0..depth {
                            string.push(delim);
                        }
                    }
                },
                StrLit { delim, expand, depth } => match (delim, expand) {
                    ('\'', false) | ('"' | '`', true) => {
                        string.push(delim);
                    }
                    _ => {
                        string.push('%');
                        string.push(if expand { 'Q' } else { 'q' });
                        for _ in 0..depth {
                            string.push(delim);
                        }
                    }
                },
                SymbolLit { delim, expand, depth } => match (delim, expand) {
                    ('\'', false) | ('"', true) => {
                        string.push(':');
                        string.push(delim);
                    }
                    _ => {
                        string.push('%');
                        string.push(if expand { 'I' } else { 'i' });
                        for _ in 0..depth {
                            string.push(delim);
                        }
                    }
                },
                HeredocLabel { label: Some(label), expand } => {
                    string.push('\0');
                    string.push(if expand { '"' } else { '\'' });
                    string.push_str(label);
                    string.push(if expand { '"' } else { '\'' });
                }
                Heredoc { label, trailing_context, expand, open } => {
                    if open {
                        string.push('\0');
                        string.push(if expand { '"' } else { '\'' });
                        string.push_str(label);
                        string.push(if expand { '"' } else { '\'' });
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

#[derive(Clone, Copy)]
struct Token<'a> {
    kind: TokenKind<'a>,
    start: usize,
    end: usize,
}

#[derive(Clone, Copy)]
#[rustfmt::skip]
enum TokenKind<'a> {
    BuiltinMethod { takes_arg: bool },
    CloseBrace,
    CloseExpansion { kind: ExpansionKind<'a> },
    ColonColon,
    Comma,
    Comment,
    Def,
    Do,
    Dot,
    Heredoc { label: &'a str, trailing_context: &'a str, expand: bool, open: bool },
    HeredocLabel { label: Option<&'a str>, expand: bool },
    Ident,
    Key,
    Keyword { takes_expr: bool },
    Method,
    MethodOwner,
    OpenBrace,
    OpenBracket,
    OpenExpansion { kind: ExpansionKind<'a> },
    OpenParen,
    Op,
    Punct,
    PureSymbolLit,
    RegexpLit { delim: char, depth: usize, expand: bool },
    Semi,
    StrLit { delim: char, depth: usize, expand: bool },
    SymbolLit { delim: char, depth: usize, expand: bool },
    UpperIdent,
    Variable,
}

#[derive(Clone, Copy)]
#[rustfmt::skip]
enum ExpansionKind<'a> {
    InHeredoc { label: &'a str, trailing_context: &'a str },
    InRegexp { delim: char, depth: usize },
    InStr { delim: char, depth: usize },
    InSymbol { delim: char, depth: usize },
}

struct Tokens<'a> {
    text: &'a str,
    context: &'a str,
    chars: Peekable<Chain<Zip<Repeat<bool>, CharIndices<'a>>, Zip<Repeat<bool>, CharIndices<'a>>>>,
    prev: Option<Token<'a>>,
    braces: Vec<TokenKind<'a>>,
}

impl<'a> Tokens<'a> {
    fn from(text: &'a str, context: &'a str) -> Self {
        Self {
            text,
            context,
            chars: iter::repeat(true)
                .zip(context.char_indices())
                .chain(iter::repeat(false).zip(text.char_indices()))
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
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(CloseExpansion { kind }) = self.prev.map(|t| t.kind) {
            let start = self
                .chars
                .peek()
                .map_or(self.text.len(), |&(_, (idx, _))| idx);
            #[rustfmt::skip]
            let kind = match kind {
                InHeredoc { label, trailing_context } => self.heredoc_resume(label, trailing_context),
                InRegexp { delim, depth } => self.regexp_lit(delim, depth, true),
                InStr { delim, depth } => self.str_lit(delim, depth, true),
                InSymbol { delim, depth } => self.symbol_lit(delim, depth, true),
            };
            let end = self
                .chars
                .peek()
                .map_or(self.text.len(), |&(_, (idx, _))| idx);

            let token = Token { kind, start, end };
            self.prev.replace(token);

            return Some(token);
        }

        let (start, ch) = match self.chars.find(|&(_, (_, ch))| !ch.is_ascii_whitespace())? {
            (true, (_, ch)) => (0, ch),
            (false, (idx, ch)) => (idx, ch),
        };

        let kind = match ch {
            // comment or expression expansion
            #[rustfmt::skip]
            '#' => match self.prev.map(|t| t.kind) {
                Some(Heredoc { label, trailing_context, open: true, .. }) => {
                    self.chars.next();
                    OpenExpansion { kind: InHeredoc { label, trailing_context } }
                }
                Some(RegexpLit { delim, depth, .. }) if depth > 0 => {
                    self.chars.next();
                    OpenExpansion { kind: InRegexp { delim, depth } }
                }
                Some(StrLit { delim, depth, .. }) if depth > 0 => {
                    self.chars.next();
                    OpenExpansion { kind: InStr { delim, depth } }
                }
                Some(SymbolLit { delim, depth, .. }) if depth > 0 => {
                    self.chars.next();
                    OpenExpansion { kind: InSymbol { delim, depth } }
                }
                _ => self.comment(),
            },

            // string
            '\'' => self.str_lit(ch, 1, false),
            '"' | '`' => self.str_lit(ch, 1, true),

            // symbol
            ':' => match self.chars.peek() {
                Some(&(_, (_, ' ' | '\t'))) | None => Op,
                Some(&(_, (_, ':'))) => {
                    self.chars.next();
                    ColonColon
                }
                Some(&(_, (_, '\''))) => {
                    self.chars.next();
                    self.symbol_lit('\'', 1, false)
                }
                Some(&(_, (_, '"'))) => {
                    self.chars.next();
                    self.symbol_lit('"', 1, true)
                }
                Some(&(_, (_, ch))) => self.pure_symbol_lit(ch),
            },

            // regexp
            '/' => match self.prev.map(|t| t.kind) {
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
                ) => self.regexp_lit(ch, 1, true),
                Some(Def | Dot) => self.method(ch),
                Some(BuiltinMethod { takes_arg: true } | Ident) => {
                    match self.prev.filter(|t| t.end == start) {
                        Some(_) => Op,
                        None => match self.chars.peek() {
                            Some(&(_, (_, ' ' | '\t'))) | None => Op,
                            _ => self.regexp_lit(ch, 1, true),
                        },
                    }
                }
                Some(_) => Op,
                None => self.regexp_lit(ch, 1, true),
            },

            // percent literal
            '%' => match self.prev.map(|t| t.kind) {
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
                    match self.prev.filter(|t| t.end == start) {
                        Some(_) => Op,
                        None => self.percent_lit(),
                    }
                }
                Some(_) => Op,
                None => self.percent_lit(),
            },

            // operator
            '?' => Op,
            '<' => match self.prev.map(|t| t.kind) {
                Some(Def | Dot) => self.method(ch),
                _ => self.heredoc_label(),
            },
            '!' | '&' | '*' | '+' | '-' | '=' | '>' | '^' | '|' | '~' => {
                match self.prev.map(|t| t.kind) {
                    Some(Def | Dot) => self.method(ch),
                    _ => Op,
                }
            }
            '.' => match self.chars.next_if(|&(_, (_, ch))| ch == '.') {
                Some(_) => {
                    self.chars.next_if(|&(_, (_, ch))| ch == '.');
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
            '[' => match self.prev.map(|t| t.kind) {
                Some(Def | Dot) => self.method(ch),
                _ => OpenBracket,
            },
            '(' => OpenParen,
            '}' => match self.braces.last() {
                Some(&OpenExpansion { kind }) => CloseExpansion { kind },
                _ => CloseBrace,
            },
            ch if is_delim(ch) => Punct,

            // heredoc
            '\0' => self.heredoc(),

            // identifier or keyword
            ch if ch.is_ascii_uppercase() => match self.prev.map(|t| t.kind) {
                Some(Def) => self.method_owner_or_method(),
                Some(Dot) => self.method(ch),
                _ => self.upper_ident(),
            },
            _ => match self.prev.map(|t| t.kind) {
                Some(Def) => self.method_owner_or_method(),
                Some(Dot) => self.method(ch),
                _ => self.ident_or_keyword(start),
            },
        };

        let end = match self.chars.peek() {
            Some(&(true, _)) => 0,
            Some(&(false, (idx, _))) => idx,
            None => self.text.len(),
        };

        let token = Token { kind, start, end };
        self.prev.replace(token);
        match token.kind {
            OpenBrace | OpenExpansion { .. } => {
                self.braces.push(token.kind);
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
    fn consume_content(&mut self, delim: char, depth: usize, expand: bool) -> usize {
        let close_delim = match delim {
            '(' => ')',
            '<' => '>',
            '[' => ']',
            '{' => '}',
            _ => delim,
        };
        let mut depth = depth;
        while let Some(&(_, (_, ch))) = self.chars.peek() {
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
                    if expand && matches!(self.chars.clone().nth(1), Some((_, (_, '{')))) {
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

    fn comment(&mut self) -> TokenKind<'a> {
        while self.chars.next().is_some() {}
        Comment
    }

    #[rustfmt::skip]
    fn str_lit(&mut self, delim: char, depth: usize, expand: bool) -> TokenKind<'a> {
        let depth = self.consume_content(delim, depth, expand);
        StrLit { delim, depth, expand }
    }

    #[rustfmt::skip]
    fn symbol_lit(&mut self, delim: char, depth: usize, expand: bool) -> TokenKind<'a> {
        let depth = self.consume_content(delim, depth, expand);
        SymbolLit { delim, depth, expand }
    }

    fn pure_symbol_lit(&mut self, peeked: char) -> TokenKind<'a> {
        let valid = match peeked {
            '!' | '%' | '&' | '*' | '+' | '-' | '/' | '<' | '>' | '^' | '|' | '~' => true,
            '=' => matches!(self.chars.clone().nth(1), Some((_, (_, '=' | '~')))),
            '[' => matches!(self.chars.clone().nth(1), Some((_, (_, ']')))),
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
    fn regexp_lit(&mut self, delim: char, depth: usize, expand: bool) -> TokenKind<'a> {
        let depth = self.consume_content(delim, depth, expand);
        while let Some(&(_, (_, 'i' | 'm' | 'o' | 'x'))) = self.chars.peek() {
            self.chars.next();
        }
        RegexpLit { delim, depth, expand }
    }

    fn percent_lit(&mut self) -> TokenKind<'a> {
        match self.chars.peek() {
            Some(&(_, (_, ch))) if ch.is_ascii_punctuation() => {
                self.chars.next();
                self.str_lit(ch, 1, true)
            }
            Some(&(_, (_, 'q' | 'w'))) => match self.chars.clone().nth(1) {
                Some((_, (_, ch))) if ch.is_ascii_punctuation() => {
                    self.chars.next();
                    self.chars.next();
                    self.str_lit(ch, 1, false)
                }
                _ => Op,
            },
            Some(&(_, (_, 'Q' | 'W' | 'x'))) => match self.chars.clone().nth(1) {
                Some((_, (_, ch))) if ch.is_ascii_punctuation() => {
                    self.chars.next();
                    self.chars.next();
                    self.str_lit(ch, 1, true)
                }
                _ => Op,
            },
            Some(&(_, (_, 'i' | 's'))) => match self.chars.clone().nth(1) {
                Some((_, (_, ch))) if ch.is_ascii_punctuation() => {
                    self.chars.next();
                    self.chars.next();
                    self.symbol_lit(ch, 1, false)
                }
                _ => Op,
            },
            Some(&(_, (_, 'I'))) => match self.chars.clone().nth(1) {
                Some((_, (_, ch))) if ch.is_ascii_punctuation() => {
                    self.chars.next();
                    self.chars.next();
                    self.symbol_lit(ch, 1, true)
                }
                _ => Op,
            },
            Some(&(_, (_, 'r'))) => match self.chars.clone().nth(1) {
                Some((_, (_, ch))) if ch.is_ascii_punctuation() => {
                    self.chars.next();
                    self.chars.next();
                    self.regexp_lit(ch, 1, true)
                }
                _ => Op,
            },
            _ => Op,
        }
    }

    #[rustfmt::skip]
    fn heredoc_label(&mut self) -> TokenKind<'a> {
        match self.chars.peek() {
            Some(&(_, (_, '<'))) => match self.chars.clone().nth(1) {
                Some((_, (start, ch))) if !is_delim(ch) => {
                    self.chars.nth(1);
                    while self.chars.next_if(|&(_, (_, ch))| !is_delim(ch)).is_some() {}
                    let end = self
                        .chars
                        .peek()
                        .map_or(self.text.len(), |&(_, (idx, _))| idx);
                    let label = Some(&self.text[start..end]);
                    HeredocLabel { label, expand: true }
                }
                Some((_, (start, d @ '\'' | d @ '"' | d @ '`'))) => {
                    self.chars.nth(1);
                    let label = self
                        .chars
                        .find(|&(_, (_, ch))| ch == d)
                        .map(|(_, (end, _))| &self.text[(start + 1)..end]);
                    HeredocLabel { label, expand: d != '\'' }
                }
                Some((_, (_, '-' | '~'))) => match self.chars.clone().nth(2) {
                    Some((_, (start, ch))) if !is_delim(ch) => {
                        self.chars.nth(2);
                        while self.chars.next_if(|&(_, (_, ch))| !is_delim(ch)).is_some() {}
                        let end = self
                            .chars
                            .peek()
                            .map_or(self.text.len(), |&(_, (idx, _))| idx);
                        let label = Some(&self.text[start..end]);
                        HeredocLabel { label, expand: true }
                    }
                    Some((_, (start, d @ '\'' | d @ '"' | d @ '`'))) => {
                        self.chars.nth(2);
                        let label = self
                            .chars
                            .find(|&(_, (_, ch))| ch == d)
                            .map(|(_, (end, _))| &self.text[(start + 1)..end]);
                        HeredocLabel { label, expand: d != '\'' }
                    }
                    _ => Op,
                },
                _ => Op,
            },
            _ => Op,
        }
    }

    #[rustfmt::skip]
    fn heredoc(&mut self) -> TokenKind<'a> {
        let delim = self.chars.next().map(|(_, (_, ch))| ch).unwrap();
        let expand = delim == '"';

        let start = self.chars.peek().map(|&(_, (idx, _))| idx).unwrap();
        let end = self
            .chars
            .find(|&(_, (_, ch))| ch == delim)
            .map(|(_, (idx, _))| idx)
            .unwrap();
        let label = &self.context[start..end];

        let start = end + 1;
        while let Some(&(true, (end, ch))) = self.chars.peek() {
            if ch == '#' {
                let trailing_context = &self.context[start..end];
                return Heredoc { label, trailing_context, expand, open: true };
            }
            self.chars.next();
        }
        let trailing_context = &self.context[start..];

        while let Some(&(_, (_, ch))) = self.chars.peek() {
            if expand && ch == '#' && matches!(self.chars.clone().nth(1), Some((_, (_, '{')))) {
                return Heredoc { label, trailing_context, expand, open: true };
            }
            self.chars.next();
        }
        let open = !self.text.contains(&label) || self.text.trim() != label.trim();
        Heredoc { label, trailing_context, expand, open }
    }

    #[rustfmt::skip]
    fn heredoc_resume(&mut self, label: &'a str, trailing_context: &'a str) -> TokenKind<'a> {
        while let Some(&(_, (_, ch))) = self.chars.peek() {
            if ch == '#' && matches!(self.chars.clone().nth(1), Some((_, (_, '{')))) {
                return Heredoc { label, trailing_context, expand: true, open: true };
            }
            self.chars.next();
        }
        Heredoc { label, trailing_context, expand: true, open: true }
    }

    fn variable(&mut self) -> TokenKind<'a> {
        self.chars.next_if(|&(_, (_, ch))| ch == '@');
        while self.chars.next_if(|&(_, (_, ch))| !is_delim(ch)).is_some() {}
        Variable
    }

    fn method_owner_or_method(&mut self) -> TokenKind<'a> {
        while self.chars.next_if(|&(_, (_, ch))| !is_delim(ch)).is_some() {}
        match self.chars.peek() {
            Some(&(_, (_, '!' | '?' | '='))) => {
                self.chars.next();
                Method
            }
            Some(&(_, (_, '.'))) => MethodOwner,
            _ => Method,
        }
    }

    fn method(&mut self, ch: char) -> TokenKind<'a> {
        match ch {
            '|' | '^' | '&' | '/' | '%' | '~' | '`' => (),
            '+' | '-' => {
                self.chars.next_if(|&(_, (_, ch))| ch == '@');
            }
            '*' => {
                self.chars.next_if(|&(_, (_, ch))| ch == '*');
            }
            '<' => match self.chars.peek() {
                Some(&(_, (_, '<'))) => {
                    self.chars.next();
                }
                Some(&(_, (_, '='))) => {
                    self.chars.next();
                    self.chars.next_if(|&(_, (_, ch))| ch == '>');
                }
                _ => (),
            },
            '>' => {
                self.chars.next_if(|&(_, (_, ch))| ch == '>' || ch == '=');
            }
            '!' => {
                self.chars.next_if(|&(_, (_, ch))| ch == '=' || ch == '~');
            }
            '=' => match self.chars.peek() {
                Some(&(_, (_, '='))) => {
                    self.chars.next();
                    self.chars.next_if(|&(_, (_, ch))| ch == '=');
                }
                Some(&(_, (_, '~'))) => {
                    self.chars.next();
                }
                _ => return Op,
            },
            '[' => match self.chars.peek() {
                Some(&(_, (_, ']'))) => {
                    self.chars.next();
                    self.chars.next_if(|&(_, (_, ch))| ch == '=');
                }
                _ => return OpenBracket,
            },
            _ => {
                while self.chars.next_if(|&(_, (_, ch))| !is_delim(ch)).is_some() {}
                self.chars
                    .next_if(|&(_, (_, ch))| ch == '!' || ch == '?' || ch == '=');
            }
        }
        Method
    }

    fn upper_ident(&mut self) -> TokenKind<'a> {
        while self.chars.next_if(|&(_, (_, ch))| !is_delim(ch)).is_some() {}
        self.chars.next_if(|&(_, (_, ch))| ch == '!' || ch == '?');
        if let Some(&(_, (_, ':'))) = self.chars.peek() {
            if !matches!(self.chars.clone().nth(1), Some((_, (_, ':')))) {
                self.chars.next();
                return Key;
            }
        }
        UpperIdent
    }

    fn ident_or_keyword(&mut self, start: usize) -> TokenKind<'a> {
        while self.chars.next_if(|&(_, (_, ch))| !is_delim(ch)).is_some() {}
        self.chars.next_if(|&(_, (_, ch))| ch == '!' || ch == '?');
        if let Some(&(_, (_, ':'))) = self.chars.peek() {
            if !matches!(self.chars.clone().nth(1), Some((_, (_, ':')))) {
                self.chars.next();
                return Key;
            }
        }
        let end = self
            .chars
            .peek()
            .map_or(self.text.len(), |&(_, (idx, _))| idx);
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
