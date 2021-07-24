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

        row.faces.clear();
        row.faces
            .resize(row.string.len(), (Fg::Default, Bg::Default));
        row.trailing_bg = Bg::Default;
        row.indent_level = 0;

        while let Some(token) = tokens.next() {
            // Highlight
            let fg = match token.kind {
                BuiltinMethod { takes_arg: true } => match tokens.peek().map(|t| t.kind) {
                    Some(Comma | Dot | Op | Semi) | None => Fg::Default,
                    _ => Fg::Macro,
                },
                BuiltinMethod { takes_arg: false } => Fg::Macro,
                Comment => Fg::Comment,
                Def | Keyword { .. } => Fg::Keyword,
                Key => Fg::Macro,
                MethodDecl => Fg::Function,
                MethodOwner => Fg::Variable,
                RegexpLit { .. } | StrLit { .. } => Fg::String,
                SymbolLit { .. } => Fg::Macro,
                UpperIdent => Fg::Type,
                Variable => Fg::Variable,
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
    BuiltinMethod { takes_arg: bool },
    Comma,
    Comment,
    Def,
    DefDot,
    Dot,
    Ident,
    Key,
    Keyword { takes_expr: bool },
    MethodCall,
    MethodDecl,
    MethodOwner,
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
    Variable,
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
    ch.is_ascii_whitespace() || ch != '_' && ch.is_ascii_punctuation()
}

impl<'a> Iterator for Tokens<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        let (start, ch) = self.chars.find(|&(_, ch)| !ch.is_ascii_whitespace())?;

        let kind = match ch {
            // comment
            '#' => self.comment(),

            // string
            '\'' | '"' | '`' => self.str_lit(ch),

            // symbol
            ':' => match self.chars.next_if(|&(_, ch)| ch == ':') {
                Some(_) => Punct,
                _ => match self.chars.peek() {
                    Some(&(_, ' ' | '\t')) | None => Op,
                    _ => match self.chars.next_if(|&(_, ch)| ch == '\'' || ch == '"') {
                        Some((_, ch)) => self.symbol_lit(ch),
                        _ => self.pure_symbol_lit(),
                    },
                },
            },

            // regexp
            '/' => match self.prev {
                Some(prev) => match prev.kind {
                    Comma
                    | Key
                    | Keyword { takes_expr: true }
                    | Op
                    | OpenBrace
                    | OpenBracket
                    | OpenParen
                    | Semi => self.regexp_lit(ch),
                    Def | DefDot => self.method_decl(),
                    Dot => self.method_call(),
                    Ident => match start == prev.end {
                        true => Op,
                        false => match self.chars.peek() {
                            Some(&(_, ' ' | '\t')) | None => Op,
                            _ => self.regexp_lit(ch),
                        },
                    },
                    _ => Op,
                },
                None => self.regexp_lit(ch),
            },

            // percent literal
            '%' => match self.prev {
                Some(prev) => match prev.kind {
                    Comma
                    | Key
                    | Keyword { takes_expr: true }
                    | Op
                    | OpenBrace
                    | OpenBracket
                    | OpenParen
                    | Semi => self.percent_lit(),
                    Def | DefDot => self.method_decl(),
                    Dot => self.method_call(),
                    Ident => match start == prev.end {
                        true => Op,
                        false => match self.chars.peek() {
                            Some(&(_, ' ' | '\t')) | None => Op,
                            _ => self.percent_lit(),
                        },
                    },
                    _ => Op,
                },
                None => self.percent_lit(),
            },

            // operator
            '!' | '&' | '*' | '+' | '-' | '<' | '=' | '>' | '?' | '^' | '|' | '~' => {
                match self.prev.map(|t| t.kind) {
                    Some(Def | DefDot) => self.method_decl(),
                    Some(Dot) => self.method_call(),
                    _ => Op,
                }
            }
            '.' => match self.chars.next_if(|&(_, ch)| ch == '.') {
                Some(_) => {
                    self.chars.next_if(|&(_, ch)| ch == '.');
                    Op
                }
                _ => match self.prev.map(|t| t.kind) {
                    Some(MethodOwner) => DefDot,
                    _ => Dot,
                },
            },

            // variable
            '@' => self.variable(),

            // punctuation
            ',' => Comma,
            '{' => OpenBrace,
            '[' => match self.prev.map(|t| t.kind) {
                Some(Def | DefDot) => self.method_decl(),
                Some(Dot) => self.method_call(),
                _ => OpenBracket,
            },
            '(' => OpenParen,
            ';' => Semi,
            ch if is_delim(ch) => Punct,

            // identifier or keyword
            ch if ch.is_ascii_uppercase() => match self.prev.map(|t| t.kind) {
                Some(Def) => self.method_owner_or_method_decl(),
                Some(DefDot) => self.method_decl(),
                Some(Dot) => self.method_call(),
                _ => self.upper_ident(),
            },
            _ => match self.prev.map(|t| t.kind) {
                Some(Def) => self.method_owner_or_method_decl(),
                Some(DefDot) => self.method_decl(),
                Some(Dot) => self.method_call(),
                _ => self.ident_or_keyword(start),
            },
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
            _ => delim,
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

    #[rustfmt::skip]
    fn str_lit(&mut self, delim: char) -> TokenKind {
        let open = self.consume_content(delim);
        StrLit { open, delim: Some(delim) }
    }

    #[rustfmt::skip]
    fn symbol_lit(&mut self, delim: char) -> TokenKind {
        let open = self.consume_content(delim);
        SymbolLit { open, delim: Some(delim) }
    }

    #[rustfmt::skip]
    fn pure_symbol_lit(&mut self) -> TokenKind {
        // TODO
        while self.chars.next_if(|&(_, ch)| !is_delim(ch)).is_some() {}
        self.chars.next_if(|&(_, ch)| ch == '!' || ch == '?');
        SymbolLit { open: false, delim: None }
    }

    #[rustfmt::skip]
    fn regexp_lit(&mut self, delim: char) -> TokenKind {
        let open = self.consume_content(delim);
        while let Some(&(_, 'i' | 'm' | 'o' | 'x')) = self.chars.peek() {
            self.chars.next();
        }
        RegexpLit { open, delim: Some(delim) }
    }

    #[rustfmt::skip]
    fn percent_lit(&mut self) -> TokenKind {
        match self.chars.peek() {
            Some(&(_, 'Q' | 'W' | 'q' | 'w' | 'x')) => {
                self.chars.next();
                match self.chars.next_if(|&(_, ch)| ch.is_ascii_punctuation()) {
                    Some((_, ch)) => self.str_lit(ch),
                    _ => StrLit { open: false, delim: None },
                }
            }
            Some(&(_, 'I' | 'i' | 's')) => {
                self.chars.next();
                match self.chars.next_if(|&(_, ch)| ch.is_ascii_punctuation()) {
                    Some((_, ch)) => self.symbol_lit(ch),
                    _ => SymbolLit { open: false, delim: None },
                }
            }
            Some(&(_, 'r')) => {
                self.chars.next();
                match self.chars.next_if(|&(_, ch)| ch.is_ascii_punctuation()) {
                    Some((_, ch)) => self.regexp_lit(ch),
                    _ => RegexpLit { open: false, delim: None },
                }
            }
            Some(&(_, ch)) if ch.is_ascii_punctuation() => {
                self.chars.next();
                self.str_lit(ch)
            }
            _ => Op,
        }
    }

    fn variable(&mut self) -> TokenKind {
        self.chars.next_if(|&(_, ch)| ch == '@');
        while self.chars.next_if(|&(_, ch)| !is_delim(ch)).is_some() {}
        Variable
    }

    fn method_owner_or_method_decl(&mut self) -> TokenKind {
        // TODO: method_delim 的な
        while self.chars.next_if(|&(_, ch)| !is_delim(ch)).is_some() {}
        self.chars.next_if(|&(_, ch)| ch == '!' || ch == '?');
        match self.chars.peek() {
            Some(&(_, '.')) => MethodOwner,
            _ => MethodDecl,
        }
    }

    fn method_decl(&mut self) -> TokenKind {
        // TODO: method_delim 的な
        while self.chars.next_if(|&(_, ch)| !is_delim(ch)).is_some() {}
        self.chars.next_if(|&(_, ch)| ch == '!' || ch == '?');
        MethodDecl
    }

    fn method_call(&mut self) -> TokenKind {
        // TODO: method_delim 的な
        while self.chars.next_if(|&(_, ch)| !is_delim(ch)).is_some() {}
        self.chars.next_if(|&(_, ch)| ch == '!' || ch == '?');
        MethodCall
    }

    fn upper_ident(&mut self) -> TokenKind {
        while self.chars.next_if(|&(_, ch)| !is_delim(ch)).is_some() {}
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
            "alias" | "class" | "defined?" | "else" | "ensure" | "false" | "for" | "end" | "in"
            | "module" | "next" | "nil" | "redo" | "rescue" | "retry" | "self" | "super"
            | "then" | "true" | "undef" | "yield" => Keyword { takes_expr: false },
            "and" | "begin" | "break" | "case" | "do" | "elsif" | "if" | "not" | "or"
            | "return" | "unless" | "until" | "when" | "while" => Keyword { takes_expr: true },
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
