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
        Some("  ")
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
        let mut prev_token: Option<Token> = None;

        row.faces.clear();
        row.faces
            .resize(row.string.len(), (Fg::Default, Bg::Default));
        row.trailing_bg = Bg::Default;
        row.indent_level = 0;

        while let Some(token) = tokens.next() {
            // Highlight
            let fg = match token.kind {
                BuiltinMethod { takes_args: false } => Fg::Macro,
                BuiltinMethod { takes_args: true } => match tokens.peek().map(|t| t.kind) {
                    Some(CloseBrace | CloseExpansion { .. } | Comment | Dot | Punct) | None => {
                        Fg::Default
                    }
                    _ => Fg::Macro,
                },
                CharLit | RegexpLit { .. } | StrLit { .. } => Fg::String,
                CloseExpansion { .. } | OpenExpansion { .. } => Fg::Variable,
                Comment | Document { .. } => Fg::Comment,
                Def | Keyword { .. } => Fg::Keyword,
                Heredoc { .. } | HeredocLabel { .. } => Fg::String,
                Ident => match tokens.peek().map(|t| t.kind) {
                    Some(OpenParen { .. }) => Fg::Function,
                    _ => Fg::Default,
                },
                Key => Fg::Macro,
                Method => Fg::Function,
                MethodOwner => Fg::Variable,
                PureSymbolLit | SymbolLit { .. } => Fg::Macro,
                UpperIdent => Fg::Type,
                Variable => Fg::Variable,
                _ => Fg::Default,
            };

            for i in token.start..token.end {
                row.faces[i].0 = fg;
            }

            // Indent
            match token.kind {
                OpenBrace { lf: true }
                | OpenBracket { lf: true }
                | OpenExpansion { lf: true, .. }
                | OpenParen { lf: true } => {
                    row.indent_level += 1;
                }
                _ => (),
            }

            if prev_token.filter(|t| t.end == 0).is_some() {
                match token.kind {
                    CloseBrace => match context_v[..] {
                        [.., OpenBrace { lf: true }] => {
                            row.indent_level -= 1;
                        }
                        _ => (),
                    },
                    CloseBracket => match context_v[..] {
                        [.., OpenBracket { lf: true }] => {
                            row.indent_level -= 1;
                        }
                        _ => (),
                    },
                    CloseExpansion { .. } => match context_v[..] {
                        [.., OpenExpansion { lf: true, .. }] => {
                            row.indent_level -= 1;
                        }
                        _ => (),
                    },
                    CloseParen => match context_v[..] {
                        [.., OpenParen { lf: true }] => {
                            row.indent_level -= 1;
                        }
                        _ => (),
                    },
                    _ => (),
                }
            }

            // Derive the context of the next row
            match token.kind {
                Document { open: true }
                | Heredoc { .. }
                | HeredocLabel { label: Some(_), .. }
                | OpenBrace { .. }
                | OpenBracket { .. }
                | OpenExpansion { .. }
                | OpenParen { .. } => {
                    context_v.push(token.kind);
                }
                RegexpLit { depth, .. } | StrLit { depth, .. } | SymbolLit { depth, .. }
                    if depth > 0 =>
                {
                    context_v.push(token.kind);
                }
                CloseBrace => {
                    for (i, kind) in context_v.iter().enumerate().rev() {
                        match kind {
                            HeredocLabel { .. } => (),
                            OpenBrace { .. } => {
                                context_v.remove(i);
                                break;
                            }
                            _ => break,
                        }
                    }
                }
                CloseBracket => {
                    for (i, kind) in context_v.iter().enumerate().rev() {
                        match kind {
                            HeredocLabel { .. } => (),
                            OpenBracket { .. } => {
                                context_v.remove(i);
                                break;
                            }
                            _ => break,
                        }
                    }
                }
                CloseExpansion { .. } => {
                    for (i, kind) in context_v.iter().enumerate().rev() {
                        match kind {
                            HeredocLabel { .. } => (),
                            OpenExpansion { .. } => {
                                context_v.remove(i - 1);
                                context_v.remove(i - 1);
                                break;
                            }
                            _ => break,
                        }
                    }
                }
                CloseParen => {
                    for (i, kind) in context_v.iter().enumerate().rev() {
                        match kind {
                            HeredocLabel { .. } => (),
                            OpenParen { .. } => {
                                context_v.remove(i);
                                break;
                            }
                            _ => break,
                        }
                    }
                }
                _ => (),
            }

            prev_token = Some(token);
        }

        if let Some(
            OpenBrace { lf }
            | OpenBracket { lf }
            | OpenExpansion { lf, .. }
            | OpenParen { lf }
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
                Document { .. } => {
                    string.push_str("\0d");
                }
                HeredocLabel { label: Some(label), indent, expand } => {
                    string.push_str("\0h");
                    if indent {
                        string.push('-');
                    }
                    string.push(if expand { '"' } else { '\'' });
                    string.push_str(label);
                    string.push(if expand { '"' } else { '\'' });
                }
                Heredoc { label, trailing_context, indent, expand, open } => {
                    if open {
                        string.push_str("\0h");
                        if indent {
                            string.push('-');
                        }
                        string.push(if expand { '"' } else { '\'' });
                        string.push_str(label);
                        string.push(if expand { '"' } else { '\'' });
                    }
                    string.push_str(trailing_context);
                }
                OpenBrace { lf } => {
                    string.push_str(if lf { "{\n" } else { "{" });
                }
                OpenBracket { lf } => {
                    string.push_str(if lf { "[\n" } else { "[" });
                }
                OpenExpansion { lf, .. } => {
                    string.push_str(if lf { "#{\n" } else { "#{" });
                }
                OpenParen { lf } => {
                    string.push_str(if lf { "(\n" } else { "(" });
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
    BuiltinMethod { takes_args: bool },
    CharLit,
    CloseBrace,
    CloseBracket,
    CloseExpansion { kind: ExpansionKind<'a> },
    CloseParen,
    ColonColon,
    Comment,
    Def,
    Document { open: bool },
    Dot,
    Heredoc { label: &'a str, trailing_context: &'a str, indent: bool, expand: bool, open: bool },
    HeredocLabel { label: Option<&'a str>, indent: bool, expand: bool },
    Ident,
    Key,
    Keyword { takes_expr: bool },
    Method,
    MethodOwner,
    NumberLit,
    OpenBrace { lf: bool },
    OpenBracket { lf: bool },
    OpenExpansion { kind: ExpansionKind<'a>, lf: bool },
    OpenParen { lf: bool },
    Punct,
    PureSymbolLit,
    RegexpLit { delim: char, depth: usize, expand: bool },
    StrLit { delim: char, depth: usize, expand: bool },
    SymbolLit { delim: char, depth: usize, expand: bool },
    UpperIdent,
    Variable,
}

#[derive(Clone, Copy)]
#[rustfmt::skip]
enum ExpansionKind<'a> {
    InHeredoc { label: &'a str, trailing_context: &'a str, indent: bool },
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
            let token = self.after_expansion(kind);
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
                Some(Heredoc { label, trailing_context, indent, open: true, .. }) => {
                    self.chars.next();
                    OpenExpansion {
                        kind: InHeredoc { label, trailing_context, indent },
                        lf: self.chars.next_if(|&(_, (_, ch))| ch == '\n').is_some(),
                    }
                }
                Some(RegexpLit { delim, depth, .. }) if depth > 0 => {
                    self.chars.next();
                    OpenExpansion {
                        kind: InRegexp { delim, depth },
                        lf: self.chars.next_if(|&(_, (_, ch))| ch == '\n').is_some(),
                    }
                }
                Some(StrLit { delim, depth, .. }) if depth > 0 => {
                    self.chars.next();
                    OpenExpansion {
                        kind: InStr { delim, depth },
                        lf: self.chars.next_if(|&(_, (_, ch))| ch == '\n').is_some(),
                    }
                }
                Some(SymbolLit { delim, depth, .. }) if depth > 0 => {
                    self.chars.next();
                    OpenExpansion {
                        kind: InSymbol { delim, depth },
                        lf: self.chars.next_if(|&(_, (_, ch))| ch == '\n').is_some(),
                    }
                }
                _ => self.comment(),
            },

            // string
            '\'' => self.str_lit(ch, 1, false),
            '"' | '`' => self.str_lit(ch, 1, true),

            // symbol
            ':' => match self.chars.peek() {
                Some(&(_, (_, ' ' | '\t'))) | None => Punct,
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
                    Key
                    | Keyword { takes_expr: true }
                    | OpenBrace { .. }
                    | OpenExpansion { .. }
                    | OpenBracket { .. }
                    | OpenParen { .. }
                    | Punct,
                )
                | None => self.regexp_lit(ch, 1, true),
                Some(BuiltinMethod { takes_args: true } | Ident | Method) => {
                    match self.prev.filter(|t| t.end == start) {
                        Some(_) => Punct,
                        None => match self.chars.peek() {
                            Some(&(_, (_, ' ' | '\t'))) | None => Punct,
                            _ => self.regexp_lit(ch, 1, true),
                        },
                    }
                }
                Some(Def | Dot) => self.method(ch),
                _ => Punct,
            },

            // percent literal
            '%' => match self.prev.map(|t| t.kind) {
                Some(
                    Key
                    | Keyword { takes_expr: true }
                    | OpenBrace { .. }
                    | OpenExpansion { .. }
                    | OpenBracket { .. }
                    | OpenParen { .. }
                    | Punct,
                )
                | None => self.percent_lit(),
                Some(BuiltinMethod { takes_args: true } | Ident | Method) => {
                    match self.prev.filter(|t| t.end == start) {
                        Some(_) => Punct,
                        None => self.percent_lit(),
                    }
                }
                Some(Def | Dot) => self.method(ch),
                _ => Punct,
            },

            // char
            '?' => match self.prev.map(|t| t.kind) {
                Some(
                    Key
                    | Keyword { takes_expr: true }
                    | OpenBrace { .. }
                    | OpenExpansion { .. }
                    | OpenBracket { .. }
                    | OpenParen { .. }
                    | Punct,
                )
                | None => self.char_lit(),
                Some(BuiltinMethod { takes_args: true } | Ident | Method) => {
                    match self.prev.filter(|t| t.end == start) {
                        Some(_) => Punct,
                        None => self.char_lit(),
                    }
                }
                _ => Punct,
            },

            // number
            '0' => match self.chars.peek() {
                Some(&(_, (_, '.'))) => self.number_lit(),
                Some(&(_, (_, 'B' | 'b'))) => self.n_ary_lit(2, true),
                Some(&(_, (_, 'O' | 'o'))) => self.n_ary_lit(8, true),
                Some(&(_, (_, 'D' | 'd'))) => self.n_ary_lit(10, true),
                Some(&(_, (_, 'X' | 'x'))) => self.n_ary_lit(16, true),
                _ => self.n_ary_lit(8, false),
            },
            '1'..='9' => self.number_lit(),

            // variable
            '@' => self.instance_variable(),
            '$' => self.global_variable(),

            // embedded document
            '=' if start == 0 => self.document_begin(),

            // punctuation
            '{' => OpenBrace {
                lf: self.chars.next_if(|&(_, (_, ch))| ch == '\n').is_some(),
            },
            '[' => match self.prev.map(|t| t.kind) {
                Some(Def | Dot) => self.method(ch),
                _ => OpenBracket {
                    lf: self.chars.next_if(|&(_, (_, ch))| ch == '\n').is_some(),
                },
            },
            '(' => OpenParen {
                lf: self.chars.next_if(|&(_, (_, ch))| ch == '\n').is_some(),
            },
            '}' => match self.braces.last() {
                Some(&OpenExpansion { kind, .. }) => CloseExpansion { kind },
                _ => CloseBrace,
            },
            ']' => CloseBracket,
            ')' => CloseParen,
            '<' => match self.prev.map(|t| t.kind) {
                Some(Def | Dot) => self.method(ch),
                _ => self.heredoc_label(),
            },
            '!' | '&' | '*' | '+' | '-' | '=' | '>' | '^' | '|' | '~' => {
                match self.prev.map(|t| t.kind) {
                    Some(Def | Dot) => self.method(ch),
                    _ => Punct,
                }
            }
            '.' => match self.chars.next_if(|&(_, (_, ch))| ch == '.') {
                Some(_) => {
                    self.chars.next_if(|&(_, (_, ch))| ch == '.');
                    Punct
                }
                _ => Dot,
            },
            ch if is_delim(ch) => Punct,

            // embedded document or heredoc
            '\0' => match self.chars.next() {
                Some((_, (_, 'd'))) => self.document(),
                Some((_, (_, 'h'))) => self.heredoc(),
                _ => Punct,
            },

            // identifier
            ch if ch.is_ascii_uppercase() => match self.prev.map(|t| t.kind) {
                Some(Def) => self.method_owner_or_method(),
                Some(Dot) => self.method(ch),
                _ => self.upper_ident(start),
            },
            _ => match self.prev.map(|t| t.kind) {
                Some(Def) => self.method_owner_or_method(),
                Some(Dot) => self.method(ch),
                _ => self.ident(start),
            },
        };
        match kind {
            OpenBrace { .. } | OpenExpansion { .. } => {
                self.braces.push(kind);
            }
            CloseBrace | CloseExpansion { .. } => {
                self.braces.pop();
            }
            _ => (),
        };

        let end = match self.chars.peek() {
            Some(&(true, _)) => 0,
            Some(&(false, (idx, _))) => idx,
            None => self.text.len(),
        };

        let token = Token { kind, start, end };
        self.prev.replace(token);
        Some(token)
    }
}

impl<'a> Tokens<'a> {
    fn after_expansion(&mut self, kind: ExpansionKind<'a>) -> Token<'a> {
        let start = self
            .chars
            .peek()
            .map_or(self.text.len(), |&(_, (idx, _))| idx);
        let kind = match kind {
            InHeredoc {
                label,
                trailing_context,
                indent,
            } => self.heredoc_resume(label, trailing_context, indent),
            InRegexp { delim, depth } => self.regexp_lit(delim, depth, true),
            InStr { delim, depth } => self.str_lit(delim, depth, true),
            InSymbol { delim, depth } => self.symbol_lit(delim, depth, true),
        };
        let end = self
            .chars
            .peek()
            .map_or(self.text.len(), |&(_, (idx, _))| idx);
        Token { kind, start, end }
    }

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
                    self.chars.nth(1);
                }
                '#' if expand => match self.chars.clone().nth(1) {
                    Some((_, (_, '{'))) => return depth,
                    _ => {
                        self.chars.next();
                    }
                },
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

    fn document_begin(&mut self) -> TokenKind<'a> {
        if self.text.starts_with("=begin") {
            match self.chars.nth(5) {
                Some((_, (_, ' ' | '\t'))) | None => {
                    while self.chars.next().is_some() {}
                    Document { open: true }
                }
                _ => Punct,
            }
        } else {
            Punct
        }
    }

    fn document(&mut self) -> TokenKind<'a> {
        if self.text.starts_with("=end") {
            self.chars.nth(3);
            match self.chars.peek() {
                Some(&(_, (_, ' ' | '\t'))) | None => Document { open: false },
                _ => {
                    while self.chars.next().is_some() {}
                    Document { open: true }
                }
            }
        } else {
            while self.chars.next().is_some() {}
            Document { open: true }
        }
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
            Punct
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
                    self.chars.nth(1);
                    self.str_lit(ch, 1, false)
                }
                _ => Punct,
            },
            Some(&(_, (_, 'Q' | 'W' | 'x'))) => match self.chars.clone().nth(1) {
                Some((_, (_, ch))) if ch.is_ascii_punctuation() => {
                    self.chars.nth(1);
                    self.str_lit(ch, 1, true)
                }
                _ => Punct,
            },
            Some(&(_, (_, 'i' | 's'))) => match self.chars.clone().nth(1) {
                Some((_, (_, ch))) if ch.is_ascii_punctuation() => {
                    self.chars.nth(1);
                    self.symbol_lit(ch, 1, false)
                }
                _ => Punct,
            },
            Some(&(_, (_, 'I'))) => match self.chars.clone().nth(1) {
                Some((_, (_, ch))) if ch.is_ascii_punctuation() => {
                    self.chars.nth(1);
                    self.symbol_lit(ch, 1, true)
                }
                _ => Punct,
            },
            Some(&(_, (_, 'r'))) => match self.chars.clone().nth(1) {
                Some((_, (_, ch))) if ch.is_ascii_punctuation() => {
                    self.chars.nth(1);
                    self.regexp_lit(ch, 1, true)
                }
                _ => Punct,
            },
            _ => Punct,
        }
    }

    fn char_lit(&mut self) -> TokenKind<'a> {
        let mut clone = self.chars.clone();
        let peek1 = clone.next().map(|(_, (_, ch))| ch);
        let peek2 = clone.next().map(|(_, (_, ch))| ch);
        match (peek1, peek2) {
            (Some('\\'), Some('u')) => {
                self.chars.nth(1);
                for _ in 0..4 {
                    self.chars.next_if(|&(_, (_, ch))| ch.is_ascii_hexdigit());
                }
            }
            (Some('\\'), Some(ch)) if ch == 'C' || ch == 'M' => {
                self.chars.nth(1);
                if self.chars.next_if(|&(_, (_, ch))| ch == '-').is_none() {
                    return CharLit;
                }
                let mut clone = self.chars.clone();
                let peek1 = clone.next().map(|(_, (_, ch))| ch);
                let peek2 = clone.next().map(|(_, (_, ch))| ch);
                let c_or_m = if ch == 'C' { 'M' } else { 'C' };
                match (peek1, peek2) {
                    (Some('\\'), Some(ch)) if ch == c_or_m => {
                        self.chars.nth(1);
                        if self.chars.next_if(|&(_, (_, ch))| ch == '-').is_none() {
                            return CharLit;
                        }
                        self.chars.next_if(|&(_, (_, ch))| ch == '\\');
                        self.chars
                            .next_if(|&(_, (_, ch))| ch.is_ascii() && !ch.is_ascii_control());
                    }
                    (Some('\\'), Some(ch)) if ch.is_ascii() && !ch.is_ascii_control() => {
                        self.chars.nth(1);
                    }
                    (Some(ch), _) if ch.is_ascii() && !ch.is_ascii_control() => {
                        self.chars.next();
                    }
                    _ => (),
                }
            }
            (Some('\\'), _) => {
                self.chars.nth(1);
            }
            (Some(ch), _) if !ch.is_ascii_whitespace() => {
                self.chars.next();
            }
            _ => return Punct,
        }
        CharLit
    }

    fn number_lit(&mut self) -> TokenKind<'a> {
        let mut fractional = false;
        let mut exponential = false;
        while let Some(&(_, (_, ch))) = self.chars.peek() {
            match ch {
                '0'..='9' => {
                    self.chars.next();
                }
                '_' => match self.chars.clone().nth(1) {
                    Some((_, (_, '0'..='9'))) => {
                        self.chars.nth(1);
                    }
                    _ => return NumberLit,
                },
                '.' => match self.chars.clone().nth(1) {
                    Some((_, (_, '0'..='9'))) => {
                        self.chars.nth(1);
                        fractional = true;
                        break;
                    }
                    _ => return NumberLit,
                },
                'e' | 'E' => match (self.chars.clone().nth(1), self.chars.clone().nth(2)) {
                    (Some((_, (_, '0'..='9'))), _) => {
                        self.chars.nth(1);
                        exponential = true;
                        break;
                    }
                    (Some((_, (_, '+' | '-'))), Some((_, (_, '0'..='9')))) => {
                        self.chars.nth(2);
                        exponential = true;
                        break;
                    }
                    _ => return NumberLit,
                },
                _ => break,
            }
        }
        if fractional {
            while let Some(&(_, (_, ch))) = self.chars.peek() {
                match ch {
                    '0'..='9' => {
                        self.chars.next();
                    }
                    '_' => match self.chars.clone().nth(1) {
                        Some((_, (_, '0'..='9'))) => {
                            self.chars.nth(1);
                        }
                        _ => return NumberLit,
                    },
                    'e' | 'E' => match (self.chars.clone().nth(1), self.chars.clone().nth(2)) {
                        (Some((_, (_, '0'..='9'))), _) => {
                            self.chars.nth(1);
                            exponential = true;
                            break;
                        }
                        (Some((_, (_, '+' | '-'))), Some((_, (_, '0'..='9')))) => {
                            self.chars.nth(2);
                            exponential = true;
                            break;
                        }
                        _ => return NumberLit,
                    },
                    _ => break,
                }
            }
        }
        if exponential {
            while let Some(&(_, (_, ch))) = self.chars.peek() {
                match ch {
                    '0'..='9' => {
                        self.chars.next();
                    }
                    '_' => match self.chars.clone().nth(1) {
                        Some((_, (_, '0'..='9'))) => {
                            self.chars.nth(1);
                        }
                        _ => return NumberLit,
                    },
                    _ => break,
                }
            }
        }
        if !exponential {
            self.chars.next_if(|&(_, (_, ch))| ch == 'r');
        }
        self.chars.next_if(|&(_, (_, ch))| ch == 'i');
        NumberLit
    }

    fn n_ary_lit(&mut self, radix: u32, explicit: bool) -> TokenKind<'a> {
        if explicit {
            match self.chars.clone().nth(1) {
                Some((_, (_, ch))) if ch.to_digit(radix).is_some() => {
                    self.chars.nth(1);
                }
                _ => return NumberLit,
            }
        }
        while let Some(&(_, (_, ch))) = self.chars.peek() {
            match ch {
                ch if ch.to_digit(radix).is_some() => {
                    self.chars.next();
                }
                '_' => match self.chars.clone().nth(1) {
                    Some((_, (_, ch))) if ch.to_digit(radix).is_some() => {
                        self.chars.nth(1);
                    }
                    _ => return NumberLit,
                },
                _ => break,
            }
        }
        self.chars.next_if(|&(_, (_, ch))| ch == 'r');
        self.chars.next_if(|&(_, (_, ch))| ch == 'i');
        NumberLit
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
                    HeredocLabel { label, indent: false, expand: true }
                }
                Some((_, (start, d @ '\'' | d @ '"' | d @ '`'))) => {
                    self.chars.nth(1);
                    let label = self
                        .chars
                        .find(|&(_, (_, ch))| ch == d)
                        .map(|(_, (end, _))| &self.text[(start + 1)..end]);
                    HeredocLabel { label, indent: false, expand: d != '\'' }
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
                        HeredocLabel { label, indent: true, expand: true }
                    }
                    Some((_, (start, d @ '\'' | d @ '"' | d @ '`'))) => {
                        self.chars.nth(2);
                        let label = self
                            .chars
                            .find(|&(_, (_, ch))| ch == d)
                            .map(|(_, (end, _))| &self.text[(start + 1)..end]);
                        HeredocLabel { label, indent: true, expand: d != '\'' }
                    }
                    _ => Punct,
                },
                _ => Punct,
            },
            _ => Punct,
        }
    }

    #[rustfmt::skip]
    fn heredoc(&mut self) -> TokenKind<'a> {
        let indent = self.chars.next_if(|&(_, (_, ch))| ch == '-').is_some();
        let delim = self.chars.next().map(|(_, (_, ch))| ch).unwrap();
        let expand = delim == '"';

        let start = self.chars.peek().map(|&(_, (idx, _))| idx).unwrap();
        let end = self
            .chars
            .find(|&(_, (_, ch))| ch == delim)
            .map(|(_, (idx, _))| idx)
            .unwrap();
        let label = &self.context[start..end];

        if let Some(&(true, (_, '#'))) = self.chars.peek() {
            return Heredoc { label, trailing_context: "", indent, expand, open: true };
        }
        while self.chars.next_if(|&(in_context, _)| in_context).is_some() {}
        let trailing_context = &self.context[(end + 1)..];

        let open = if indent {
            self.text.find(label).map_or(true, |i| {
                self.text[..i].trim() != "" || self.text.len() > i + label.len()
            })
        } else {
            self.text != label
        };
        if open {
            while let Some(&(_, (_, ch))) = self.chars.peek() {
                match ch {
                    '\\' => self.chars.nth(1),
                    '#' if expand => match self.chars.clone().nth(1) {
                        Some((_, (_, '{'))) => break,
                        _ => self.chars.next(),
                    }
                    _ => self.chars.next(),
                };
            }
        } else {
            while self.chars.next().is_some() {}
        }
        Heredoc { label, trailing_context, indent, expand, open }
    }

    #[rustfmt::skip]
    fn heredoc_resume(&mut self, label: &'a str, trailing_context: &'a str, indent: bool) -> TokenKind<'a> {
        while let Some(&(_, (_, ch))) = self.chars.peek() {
            match ch {
                '\\' => self.chars.nth(1),
                '#' => match self.chars.clone().nth(1) {
                    Some((_, (_, '{'))) => break,
                    _ => self.chars.next(),
                }
                _ => self.chars.next(),
            };
        }
        Heredoc { label, trailing_context, indent, expand: true, open: true }
    }

    fn instance_variable(&mut self) -> TokenKind<'a> {
        match self.chars.peek() {
            Some(&(_, (_, ch))) if !is_delim(ch) && !ch.is_ascii_digit() => {
                self.chars.next();
                while self.chars.next_if(|&(_, (_, ch))| !is_delim(ch)).is_some() {}
                Variable
            }
            Some(&(_, (_, '@'))) => match self.chars.clone().nth(1) {
                Some((_, (_, ch))) if !is_delim(ch) && !ch.is_ascii_digit() => {
                    self.chars.nth(1);
                    while self.chars.next_if(|&(_, (_, ch))| !is_delim(ch)).is_some() {}
                    Variable
                }
                _ => Punct,
            },
            _ => Punct,
        }
    }

    fn global_variable(&mut self) -> TokenKind<'a> {
        match self.chars.peek() {
            Some(&(_, (_, '~' | '&' | '`' | '\'' | '+'))) => {
                self.chars.next();
                Variable
            }
            Some(&(_, (_, ch))) if ch.is_ascii_digit() => {
                self.chars.next();
                while self
                    .chars
                    .next_if(|&(_, (_, ch))| ch.is_ascii_digit())
                    .is_some()
                {}
                Variable
            }
            Some(&(_, (_, ch))) if !is_delim(ch) => {
                self.chars.next();
                while self.chars.next_if(|&(_, (_, ch))| !is_delim(ch)).is_some() {}
                Variable
            }
            _ => Punct,
        }
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
                _ => return Punct,
            },
            '[' => match self.chars.peek() {
                Some(&(_, (_, ']'))) => {
                    self.chars.next();
                    self.chars.next_if(|&(_, (_, ch))| ch == '=');
                }
                _ => return OpenBracket { lf: false },
            },
            _ => {
                while self.chars.next_if(|&(_, (_, ch))| !is_delim(ch)).is_some() {}
                self.chars
                    .next_if(|&(_, (_, ch))| ch == '!' || ch == '?' || ch == '=');
            }
        }
        Method
    }

    fn upper_ident(&mut self, start: usize) -> TokenKind<'a> {
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
            "BEGIN" | "END" => Keyword { takes_expr: false },
            _ => UpperIdent,
        }
    }

    fn ident(&mut self, start: usize) -> TokenKind<'a> {
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
            "alias" | "class" | "defined?" | "else" | "ensure" | "for" | "end" | "in"
            | "module" | "next" | "redo" | "rescue" | "retry" | "super" | "then" | "undef"
            | "yield" => Keyword { takes_expr: false },
            "and" | "begin" | "break" | "case" | "do" | "elsif" | "if" | "not" | "or"
            | "return" | "unless" | "until" | "when" | "while" => Keyword { takes_expr: true },
            "__callee__" | "__dir__" | "__method__" | "block_given?" | "fail" | "private"
            | "protected" | "public" | "raise" => BuiltinMethod { takes_args: false },
            "alias_method"
            | "attr"
            | "attr_accessor"
            | "attr_reader"
            | "attr_writer"
            | "catch"
            | "define_method"
            | "extend"
            | "include"
            | "lambda"
            | "loop"
            | "module_function"
            | "p"
            | "prepend"
            | "private_class_method"
            | "private_constant"
            | "proc"
            | "public_class_method"
            | "public_constant"
            | "puts"
            | "refine"
            | "require"
            | "require_relative"
            | "throw"
            | "using" => BuiltinMethod { takes_args: true },
            "__ENCODING__" | "__FILE__" | "__LINE__" | "false" | "nil" | "self" | "true" => {
                Variable
            }
            _ => Ident,
        }
    }
}
