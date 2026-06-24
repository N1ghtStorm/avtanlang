mod token;

pub use token::{Keyword, Token, TokenKind};

use crate::diagnostics::Diagnostic;
use crate::source::SourceFile;

#[derive(Clone, Debug, Default)]
pub struct LexResult {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn lex(file: &SourceFile) -> LexResult {
    Lexer::new(file).lex()
}

struct Lexer<'a> {
    file: &'a SourceFile,
    text: &'a str,
    pos: usize,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Lexer<'a> {
    fn new(file: &'a SourceFile) -> Self {
        Self {
            file,
            text: file.text(),
            pos: 0,
            tokens: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn lex(mut self) -> LexResult {
        while !self.is_eof() {
            self.skip_whitespace();

            if self.is_eof() {
                break;
            }

            let start = self.pos;
            let Some(ch) = self.current_char() else {
                break;
            };

            match ch {
                ch if is_ident_start(ch) => self.lex_ident_or_keyword(),
                ch if ch.is_ascii_digit() => self.lex_number(),
                '"' => self.lex_string(),
                '\'' => self.lex_char(),
                '/' => self.lex_slash(),
                '?' => self.lex_question_or_hole(),
                '(' => self.emit_single(TokenKind::LParen, start),
                ')' => self.emit_single(TokenKind::RParen, start),
                '{' => self.emit_single(TokenKind::LBrace, start),
                '}' => self.emit_single(TokenKind::RBrace, start),
                '[' => self.emit_single(TokenKind::LBracket, start),
                ']' => self.emit_single(TokenKind::RBracket, start),
                ',' => self.emit_single(TokenKind::Comma, start),
                ';' => self.emit_single(TokenKind::Semicolon, start),
                '#' => self.emit_single(TokenKind::Hash, start),
                '@' => self.emit_single(TokenKind::At, start),
                '.' => self.lex_dot(),
                ':' => self.lex_colon(),
                '-' => self.lex_minus(),
                '=' => self.lex_equals(),
                '!' => self.lex_bang(),
                '<' => self.lex_less(),
                '>' => self.lex_greater(),
                '&' => self.lex_ampersand(),
                '|' => self.lex_pipe(),
                '*' => self.lex_star(),
                '+' => self.emit_single(TokenKind::Plus, start),
                '%' => self.emit_single(TokenKind::Percent, start),
                _ => {
                    self.advance_char();
                    let span = self.file.span(start, self.pos);
                    self.diagnostics.push(
                        Diagnostic::error("AVT0001", format!("unknown character `{ch}`"))
                            .with_span(span),
                    );
                    self.tokens.push(Token::new(TokenKind::Unknown(ch), span));
                }
            }
        }

        let eof_span = self.file.span(self.pos, self.pos);
        self.tokens.push(Token::new(TokenKind::Eof, eof_span));

        LexResult {
            tokens: self.tokens,
            diagnostics: self.diagnostics,
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.current_char(), Some(ch) if ch.is_whitespace()) {
            self.advance_char();
        }
    }

    fn lex_ident_or_keyword(&mut self) {
        let start = self.pos;
        self.advance_char();
        self.consume_while(is_ident_continue);

        let text = &self.text[start..self.pos];
        let kind = if text == "_" {
            TokenKind::Underscore
        } else if let Some(keyword) = Keyword::from_ident(text) {
            TokenKind::Keyword(keyword)
        } else {
            TokenKind::Ident(text.to_string())
        };

        self.emit(kind, start);
    }

    fn lex_number(&mut self) {
        let start = self.pos;

        if self.starts_with("0x") || self.starts_with("0o") || self.starts_with("0b") {
            self.advance_ascii(2);
            self.consume_while(|ch| ch.is_ascii_alphanumeric() || ch == '_');
            let literal = self.text[start..self.pos].to_string();
            self.emit(TokenKind::IntLiteral(literal), start);
            return;
        }

        self.consume_while(|ch| ch.is_ascii_digit() || ch == '_');

        let mut is_float = false;
        if self.starts_with(".")
            && !self.starts_with("..")
            && self
                .peek_ascii_after(1)
                .is_some_and(|ch| ch.is_ascii_digit())
        {
            is_float = true;
            self.advance_ascii(1);
            self.consume_while(|ch| ch.is_ascii_digit() || ch == '_');
        }

        if self.current_char().is_some_and(|ch| ch == 'e' || ch == 'E') && self.exponent_is_valid()
        {
            is_float = true;
            self.advance_char();
            if self.current_char().is_some_and(|ch| ch == '+' || ch == '-') {
                self.advance_char();
            }
            self.consume_while(|ch| ch.is_ascii_digit() || ch == '_');
        }

        let literal = self.text[start..self.pos].to_string();
        let kind = if is_float {
            TokenKind::FloatLiteral(literal)
        } else {
            TokenKind::IntLiteral(literal)
        };
        self.emit(kind, start);
    }

    fn lex_string(&mut self) {
        let start = self.pos;
        self.advance_char();

        let mut escaped = false;
        while let Some(ch) = self.current_char() {
            if !escaped && ch == '"' {
                self.advance_char();
                let literal = self.text[start + 1..self.pos - 1].to_string();
                self.emit(TokenKind::StringLiteral(literal), start);
                return;
            }

            escaped = !escaped && ch == '\\';
            self.advance_char();
        }

        let span = self.file.span(start, self.pos);
        self.diagnostics
            .push(Diagnostic::error("AVT0002", "unterminated string literal").with_span(span));
        let literal = self.text[start + 1..self.pos].to_string();
        self.emit(TokenKind::StringLiteral(literal), start);
    }

    fn lex_char(&mut self) {
        let start = self.pos;
        self.advance_char();

        let mut escaped = false;
        while let Some(ch) = self.current_char() {
            if !escaped && ch == '\'' {
                self.advance_char();
                let literal = self.text[start + 1..self.pos - 1].to_string();
                self.emit(TokenKind::CharLiteral(literal), start);
                return;
            }

            escaped = !escaped && ch == '\\';
            self.advance_char();
        }

        let span = self.file.span(start, self.pos);
        self.diagnostics
            .push(Diagnostic::error("AVT0003", "unterminated character literal").with_span(span));
        let literal = self.text[start + 1..self.pos].to_string();
        self.emit(TokenKind::CharLiteral(literal), start);
    }

    fn lex_slash(&mut self) {
        let start = self.pos;

        if self.starts_with("///") || self.starts_with("//!") {
            self.advance_ascii(3);
            self.consume_until_newline();
            let comment = self.text[start + 3..self.pos].to_string();
            self.emit(TokenKind::DocLineComment(comment), start);
        } else if self.starts_with("//") {
            self.advance_ascii(2);
            self.consume_until_newline();
            let comment = self.text[start + 2..self.pos].to_string();
            self.emit(TokenKind::LineComment(comment), start);
        } else if self.starts_with("/**") || self.starts_with("/*!") {
            self.lex_block_comment(start, true);
        } else if self.starts_with("/*") {
            self.lex_block_comment(start, false);
        } else {
            self.emit_single(TokenKind::Slash, start);
        }
    }

    fn lex_block_comment(&mut self, start: usize, is_doc: bool) {
        self.advance_ascii(2);
        let mut depth = 1usize;

        while !self.is_eof() {
            if self.starts_with("/*") {
                depth += 1;
                self.advance_ascii(2);
            } else if self.starts_with("*/") {
                depth -= 1;
                self.advance_ascii(2);
                if depth == 0 {
                    let comment = self.text[start + 2..self.pos - 2].to_string();
                    let kind = if is_doc {
                        TokenKind::DocBlockComment(comment)
                    } else {
                        TokenKind::BlockComment(comment)
                    };
                    self.emit(kind, start);
                    return;
                }
            } else {
                self.advance_char();
            }
        }

        let span = self.file.span(start, self.pos);
        self.diagnostics
            .push(Diagnostic::error("AVT0004", "unterminated block comment").with_span(span));
        let comment = self.text[start + 2..self.pos].to_string();
        let kind = if is_doc {
            TokenKind::DocBlockComment(comment)
        } else {
            TokenKind::BlockComment(comment)
        };
        self.emit(kind, start);
    }

    fn lex_question_or_hole(&mut self) {
        let start = self.pos;
        self.advance_char();

        if self.current_char().is_some_and(is_ident_start) {
            let ident_start = self.pos;
            self.advance_char();
            self.consume_while(is_ident_continue);
            let name = self.text[ident_start..self.pos].to_string();
            self.emit(TokenKind::HoleIdent(name), start);
        } else {
            self.emit(TokenKind::Question, start);
        }
    }

    fn lex_dot(&mut self) {
        let start = self.pos;
        if self.starts_with("..") {
            self.advance_ascii(2);
            self.emit(TokenKind::DotDot, start);
        } else {
            self.emit_single(TokenKind::Dot, start);
        }
    }

    fn lex_colon(&mut self) {
        let start = self.pos;
        if self.starts_with("::") {
            self.advance_ascii(2);
            self.emit(TokenKind::DoubleColon, start);
        } else {
            self.emit_single(TokenKind::Colon, start);
        }
    }

    fn lex_minus(&mut self) {
        let start = self.pos;
        if self.starts_with("->") {
            self.advance_ascii(2);
            self.emit(TokenKind::Arrow, start);
        } else {
            self.emit_single(TokenKind::Minus, start);
        }
    }

    fn lex_equals(&mut self) {
        let start = self.pos;
        if self.starts_with("=>") {
            self.advance_ascii(2);
            self.emit(TokenKind::FatArrow, start);
        } else if self.starts_with("==") {
            self.advance_ascii(2);
            self.emit(TokenKind::EqEq, start);
        } else {
            self.emit_single(TokenKind::Eq, start);
        }
    }

    fn lex_bang(&mut self) {
        let start = self.pos;
        if self.starts_with("!=") {
            self.advance_ascii(2);
            self.emit(TokenKind::BangEq, start);
        } else {
            self.emit_single(TokenKind::Bang, start);
        }
    }

    fn lex_less(&mut self) {
        let start = self.pos;
        if self.starts_with("<=") {
            self.advance_ascii(2);
            self.emit(TokenKind::LtEq, start);
        } else {
            self.emit_single(TokenKind::Lt, start);
        }
    }

    fn lex_greater(&mut self) {
        let start = self.pos;
        if self.starts_with(">=") {
            self.advance_ascii(2);
            self.emit(TokenKind::GtEq, start);
        } else {
            self.emit_single(TokenKind::Gt, start);
        }
    }

    fn lex_ampersand(&mut self) {
        let start = self.pos;
        if self.starts_with("&&") {
            self.advance_ascii(2);
            self.emit(TokenKind::AmpAmp, start);
        } else {
            self.emit_single(TokenKind::Amp, start);
        }
    }

    fn lex_pipe(&mut self) {
        let start = self.pos;
        if self.starts_with("||") {
            self.advance_ascii(2);
            self.emit(TokenKind::PipePipe, start);
        } else {
            self.emit_single(TokenKind::Pipe, start);
        }
    }

    fn lex_star(&mut self) {
        let start = self.pos;
        if self.starts_with("**") {
            self.advance_ascii(2);
            self.emit(TokenKind::DoubleStar, start);
        } else {
            self.emit_single(TokenKind::Star, start);
        }
    }

    fn exponent_is_valid(&self) -> bool {
        let mut chars = self.text[self.pos..].chars();
        let Some('e' | 'E') = chars.next() else {
            return false;
        };

        match chars.next() {
            Some('+' | '-') => chars.next().is_some_and(|ch| ch.is_ascii_digit()),
            Some(ch) => ch.is_ascii_digit(),
            None => false,
        }
    }

    fn consume_until_newline(&mut self) {
        while let Some(ch) = self.current_char() {
            if ch == '\n' || ch == '\r' {
                break;
            }
            self.advance_char();
        }
    }

    fn consume_while(&mut self, predicate: impl Fn(char) -> bool) {
        while self.current_char().is_some_and(&predicate) {
            self.advance_char();
        }
    }

    fn emit_single(&mut self, kind: TokenKind, start: usize) {
        self.advance_char();
        self.emit(kind, start);
    }

    fn emit(&mut self, kind: TokenKind, start: usize) {
        let span = self.file.span(start, self.pos);
        self.tokens.push(Token::new(kind, span));
    }

    fn current_char(&self) -> Option<char> {
        self.text.get(self.pos..)?.chars().next()
    }

    fn peek_ascii_after(&self, offset: usize) -> Option<char> {
        self.text.get(self.pos + offset..)?.chars().next()
    }

    fn starts_with(&self, pattern: &str) -> bool {
        self.text[self.pos..].starts_with(pattern)
    }

    fn advance_char(&mut self) -> Option<char> {
        let ch = self.current_char()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn advance_ascii(&mut self, count: usize) {
        debug_assert!(
            self.text[self.pos..]
                .bytes()
                .take(count)
                .all(|b| b.is_ascii())
        );
        self.pos += count;
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.text.len()
    }
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

#[cfg(test)]
mod tests {
    use crate::source::{FileId, SourceFile};

    use super::*;

    fn lex_text(text: &str) -> LexResult {
        let file = SourceFile::new(FileId(0), "test.avtn", text);
        lex(&file)
    }

    #[test]
    fn lexes_dependent_enum_syntax() {
        let result = lex_text(
            "enum Vect<T, const N: Nat> {\n    Nil where N == Z,\n    Cons { head: T, tail: Vect<T, N> },\n}\n",
        );

        assert!(result.diagnostics.is_empty());
        let kinds: Vec<_> = result.tokens.into_iter().map(|token| token.kind).collect();

        assert_eq!(kinds[0], TokenKind::Keyword(Keyword::Enum));
        assert_eq!(kinds[1], TokenKind::Ident("Vect".to_string()));
        assert_eq!(kinds[2], TokenKind::Lt);
        assert!(kinds.contains(&TokenKind::Keyword(Keyword::Const)));
        assert!(kinds.contains(&TokenKind::Gt));
        assert!(kinds.contains(&TokenKind::Keyword(Keyword::Where)));
        assert!(kinds.contains(&TokenKind::EqEq));
    }

    #[test]
    fn lexes_implicit_erased_holes_and_rewrite() {
        let result = lex_text("proof fn p {erased n: Nat} -> x == y { rewrite ?step in Refl }\n");

        assert!(result.diagnostics.is_empty());
        let kinds: Vec<_> = result.tokens.into_iter().map(|token| token.kind).collect();

        assert!(kinds.contains(&TokenKind::Keyword(Keyword::Proof)));
        assert!(kinds.contains(&TokenKind::Keyword(Keyword::Erased)));
        assert!(kinds.contains(&TokenKind::Keyword(Keyword::Rewrite)));
        assert!(kinds.contains(&TokenKind::HoleIdent("step".to_string())));
    }

    #[test]
    fn reports_unterminated_block_comment() {
        let result = lex_text("/* open");

        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "AVT0004");
        assert_eq!(
            result.tokens[0].kind,
            TokenKind::BlockComment(" open".to_string())
        );
    }
}
