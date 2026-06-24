pub mod ast;
pub mod dump;

pub use ast::*;
pub use dump::dump_module;

use crate::diagnostics::Diagnostic;
use crate::lexer::{Keyword, Token, TokenKind};
use crate::source::{FileId, Span};

#[derive(Clone, Debug)]
pub struct ParseResult {
    pub module: Module,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn parse_tokens(tokens: &[Token]) -> ParseResult {
    Parser::new(tokens).parse_module()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stop {
    Comma,
    Eq,
    LBrace,
    RParen,
    RBrace,
    RBracket,
    Gt,
    Semicolon,
    Keyword(Keyword),
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Parser {
    fn new(tokens: &[Token]) -> Self {
        let tokens = tokens
            .iter()
            .filter(|token| !is_trivia(&token.kind))
            .cloned()
            .collect();

        Self {
            tokens,
            pos: 0,
            diagnostics: Vec::new(),
        }
    }

    fn parse_module(mut self) -> ParseResult {
        let package = if self.eat_keyword(Keyword::Package).is_some() {
            Some(self.parse_package_path())
        } else {
            None
        };

        let mut items = Vec::new();
        while !self.at_eof() {
            items.push(self.parse_item());
        }

        ParseResult {
            module: Module { package, items },
            diagnostics: self.diagnostics,
        }
    }

    fn parse_item(&mut self) -> Item {
        let attributes = self.parse_attributes();
        let public = self.eat_keyword(Keyword::Pub).is_some();
        let flavor = self.parse_fn_flavor();

        if self.at_keyword(Keyword::Import) {
            return Item::Import(self.parse_import(attributes));
        }
        if self.at_keyword(Keyword::Enum) {
            return Item::Enum(self.parse_enum(attributes, public));
        }
        if self.at_keyword(Keyword::Struct) {
            return Item::Struct(self.parse_struct(attributes, public));
        }
        if self.at_keyword(Keyword::Type) {
            return Item::TypeAlias(self.parse_type_alias(attributes, public));
        }
        if self.at_keyword(Keyword::Fn) {
            return Item::Fn(self.parse_fn(attributes, public, flavor));
        }
        if self.at_keyword(Keyword::Impl) {
            return Item::Impl(self.parse_impl(attributes));
        }

        let span = self.current_span();
        self.error_here("AVP0001", "expected item");
        self.bump();
        self.synchronize_item();
        Item::Error(span)
    }

    fn parse_import(&mut self, attributes: Vec<Attribute>) -> ImportItem {
        let start = self
            .expect_keyword(Keyword::Import, "expected `import`")
            .start;
        let (path, members) = self.parse_import_path();
        let alias = if self.eat_keyword(Keyword::As).is_some() {
            Some(
                self.expect_ident("expected import alias")
                    .unwrap_or_default(),
            )
        } else {
            None
        };
        let end = self
            .eat_kind(&TokenKind::Semicolon)
            .unwrap_or(path.span)
            .end;

        ImportItem {
            attributes,
            path,
            members,
            alias,
            span: self.span(start, end),
        }
    }

    fn parse_import_path(&mut self) -> (Path, Vec<String>) {
        let start = self.current_span().start;
        let mut segments = Vec::new();
        let mut members = Vec::new();

        segments.push(
            self.expect_ident("expected import path")
                .unwrap_or_default(),
        );
        while self.eat_kind(&TokenKind::Dot).is_some() {
            if self.at_kind(&TokenKind::LBrace) {
                members = self.parse_import_members();
                break;
            }
            segments.push(
                self.expect_ident("expected import path segment")
                    .unwrap_or_default(),
            );
        }

        if self.eat_kind(&TokenKind::DoubleColon).is_some() {
            if self.at_kind(&TokenKind::LBrace) {
                members = self.parse_import_members();
            } else {
                segments.push(
                    self.expect_ident("expected import path segment")
                        .unwrap_or_default(),
                );
            }
        }

        (
            Path::new(segments, self.span(start, self.previous_end())),
            members,
        )
    }

    fn parse_import_members(&mut self) -> Vec<String> {
        self.expect_kind(&TokenKind::LBrace, "expected `{` in import group");
        let mut members = Vec::new();

        while !self.at_eof() && !self.at_kind(&TokenKind::RBrace) {
            if self.eat_kind(&TokenKind::Comma).is_some() {
                continue;
            }
            members.push(
                self.expect_ident("expected imported member")
                    .unwrap_or_default(),
            );
            self.eat_kind(&TokenKind::Comma);
        }

        self.expect_kind(&TokenKind::RBrace, "expected `}` after import group");
        members
    }

    fn parse_enum(&mut self, attributes: Vec<Attribute>, public: bool) -> EnumItem {
        let start = self.expect_keyword(Keyword::Enum, "expected `enum`").start;
        let name = self.expect_ident("expected enum name").unwrap_or_default();
        let generics = self.parse_generic_params();
        self.expect_kind(&TokenKind::LBrace, "expected `{` after enum name");

        let mut variants = Vec::new();
        while !self.at_eof() && !self.at_kind(&TokenKind::RBrace) {
            if self.eat_kind(&TokenKind::Comma).is_some() {
                continue;
            }
            variants.push(self.parse_enum_variant());
        }

        let end = self
            .expect_kind(&TokenKind::RBrace, "expected `}` after enum body")
            .end;

        EnumItem {
            attributes,
            public,
            name,
            generics,
            variants,
            span: self.span(start, end),
        }
    }

    fn parse_enum_variant(&mut self) -> EnumVariant {
        let attributes = self.parse_attributes();
        let start = self.current_span().start;
        let name = self
            .expect_ident("expected enum variant name")
            .unwrap_or_default();
        let generics = self.parse_generic_params();
        let kind = if self.at_kind(&TokenKind::LParen) {
            VariantKind::Tuple(self.parse_tuple_fields())
        } else if self.at_kind(&TokenKind::LBrace) {
            VariantKind::Struct(self.parse_named_fields())
        } else {
            VariantKind::Unit
        };

        let mut where_clauses = Vec::new();
        while self.eat_keyword(Keyword::Where).is_some() {
            where_clauses.push(self.parse_expr(&[Stop::Comma, Stop::RBrace]));
        }

        let end = if let Some(span) = self.eat_kind(&TokenKind::Comma) {
            span.end
        } else {
            self.previous_end()
        };

        EnumVariant {
            attributes,
            name,
            generics,
            kind,
            where_clauses,
            span: self.span(start, end),
        }
    }

    fn parse_struct(&mut self, attributes: Vec<Attribute>, public: bool) -> StructItem {
        let start = self
            .expect_keyword(Keyword::Struct, "expected `struct`")
            .start;
        let name = self
            .expect_ident("expected struct name")
            .unwrap_or_default();
        let generics = self.parse_generic_params();
        let fields = if self.at_kind(&TokenKind::LBrace) {
            self.parse_named_fields()
        } else if self.at_kind(&TokenKind::LParen) {
            self.parse_tuple_fields()
        } else {
            Vec::new()
        };
        let where_clauses = self.parse_where_clauses(&[Stop::Semicolon]);
        let end = self
            .eat_kind(&TokenKind::Semicolon)
            .map(|span| span.end)
            .unwrap_or_else(|| self.previous_end());

        StructItem {
            attributes,
            public,
            name,
            generics,
            fields,
            where_clauses,
            span: self.span(start, end),
        }
    }

    fn parse_type_alias(&mut self, attributes: Vec<Attribute>, public: bool) -> TypeAliasItem {
        let start = self.expect_keyword(Keyword::Type, "expected `type`").start;
        let name = self
            .expect_ident("expected type alias name")
            .unwrap_or_default();
        let generics = self.parse_generic_params();
        self.expect_kind(&TokenKind::Eq, "expected `=` in type alias");
        let ty = self.parse_type(&[Stop::Keyword(Keyword::Where), Stop::Semicolon, Stop::RBrace]);
        let where_clauses = self.parse_where_clauses(&[Stop::Semicolon]);
        let end = self
            .eat_kind(&TokenKind::Semicolon)
            .map(|span| span.end)
            .unwrap_or(ty.span.end);

        TypeAliasItem {
            attributes,
            public,
            name,
            generics,
            ty,
            where_clauses,
            span: self.span(start, end),
        }
    }

    fn parse_fn(&mut self, attributes: Vec<Attribute>, public: bool, flavor: FnFlavor) -> FnItem {
        let start = self.expect_keyword(Keyword::Fn, "expected `fn`").start;
        let name = self
            .expect_ident("expected function name")
            .unwrap_or_default();
        let generics = self.parse_generic_params();
        let params = self.parse_params();
        let return_type = if self.eat_kind(&TokenKind::Arrow).is_some() {
            Some(self.parse_type(&[
                Stop::Keyword(Keyword::Requires),
                Stop::Keyword(Keyword::Ensures),
                Stop::LBrace,
                Stop::Semicolon,
            ]))
        } else {
            None
        };
        let requires = self.parse_contracts(Keyword::Requires);
        let ensures = self.parse_contracts(Keyword::Ensures);
        let body = if self.at_kind(&TokenKind::LBrace) {
            Some(self.parse_block())
        } else {
            self.eat_kind(&TokenKind::Semicolon);
            None
        };
        let end = body
            .as_ref()
            .map(|block| block.span.end)
            .or_else(|| return_type.as_ref().map(|ty| ty.span.end))
            .unwrap_or_else(|| self.previous_end());

        FnItem {
            attributes,
            public,
            flavor,
            name,
            generics,
            params,
            return_type,
            requires,
            ensures,
            body,
            span: self.span(start, end),
        }
    }

    fn parse_impl(&mut self, attributes: Vec<Attribute>) -> ImplItem {
        let start = self.expect_keyword(Keyword::Impl, "expected `impl`").start;
        let generics = self.parse_generic_params();
        let target = self.parse_type(&[Stop::LBrace]);
        self.expect_kind(&TokenKind::LBrace, "expected `{` after impl target");

        let mut items = Vec::new();
        while !self.at_eof() && !self.at_kind(&TokenKind::RBrace) {
            items.push(self.parse_item());
        }
        let end = self
            .expect_kind(&TokenKind::RBrace, "expected `}` after impl body")
            .end;

        ImplItem {
            attributes,
            generics,
            target,
            items,
            span: self.span(start, end),
        }
    }

    fn parse_generic_params(&mut self) -> Vec<GenericParam> {
        if self.eat_kind(&TokenKind::Lt).is_none() {
            return Vec::new();
        }

        let mut params = Vec::new();
        while !self.at_eof() && !self.at_kind(&TokenKind::Gt) {
            if self.eat_kind(&TokenKind::Comma).is_some() {
                continue;
            }

            let start = self.current_span().start;
            let is_const = self.eat_keyword(Keyword::Const).is_some();
            let name = self
                .expect_ident("expected generic parameter name")
                .unwrap_or_default();
            let kind = if is_const {
                self.expect_kind(&TokenKind::Colon, "expected `:` after const parameter");
                let ty = self.parse_type(&[Stop::Comma, Stop::Gt]);
                GenericParamKind::Const { ty }
            } else {
                GenericParamKind::Type
            };

            params.push(GenericParam {
                name,
                kind,
                span: self.span(start, self.previous_end()),
            });

            if self.eat_kind(&TokenKind::Comma).is_none() {
                break;
            }
        }

        self.expect_kind(&TokenKind::Gt, "expected `>` after generic parameters");
        params
    }

    fn parse_params(&mut self) -> Vec<Param> {
        if self.at_kind(&TokenKind::LParen) {
            return self.parse_parenthesized_params();
        }

        let mut params = Vec::new();
        while self.at_braced_param_start() {
            params.push(self.parse_braced_param());
            self.eat_kind(&TokenKind::Comma);
        }

        if params.is_empty() {
            self.expect_kind(&TokenKind::LParen, "expected `(` before parameters");
        }

        params
    }

    fn parse_parenthesized_params(&mut self) -> Vec<Param> {
        self.expect_kind(&TokenKind::LParen, "expected `(` before parameters");
        let mut params = Vec::new();

        while !self.at_eof() && !self.at_kind(&TokenKind::RParen) {
            if self.eat_kind(&TokenKind::Comma).is_some() {
                continue;
            }

            if self.at_kind(&TokenKind::LBrace) {
                params.push(self.parse_braced_param());
            } else {
                params.push(self.parse_explicit_param(&[Stop::Comma, Stop::RParen]));
            }

            if self.eat_kind(&TokenKind::Comma).is_none() {
                break;
            }
        }

        self.expect_kind(&TokenKind::RParen, "expected `)` after parameters");
        params
    }

    fn parse_explicit_param(&mut self, stops: &[Stop]) -> Param {
        let start = self.current_span().start;
        let pattern = self.parse_pattern();
        self.expect_kind(&TokenKind::Colon, "expected `:` after parameter name");
        let ty = self.parse_type(stops);
        Param {
            mode: BinderMode::Explicit,
            name: pattern,
            span: self.span(start, ty.span.end),
            ty,
        }
    }

    fn parse_braced_param(&mut self) -> Param {
        let start = self
            .expect_kind(&TokenKind::LBrace, "expected `{` before binder")
            .start;
        let mode = self.parse_braced_binder_mode();
        let pattern = self.parse_pattern();
        self.expect_kind(&TokenKind::Colon, "expected `:` after binder name");
        let ty = self.parse_type(&[Stop::RBrace]);
        let end = self
            .expect_kind(&TokenKind::RBrace, "expected `}` after binder")
            .end;

        Param {
            mode,
            name: pattern,
            ty,
            span: self.span(start, end),
        }
    }

    fn parse_braced_binder_mode(&mut self) -> BinderMode {
        if self.eat_keyword(Keyword::Auto).is_some() {
            BinderMode::Auto
        } else if self.eat_keyword(Keyword::Erased).is_some() {
            BinderMode::Erased
        } else if self.eat_keyword(Keyword::Implicit).is_some() {
            BinderMode::Implicit
        } else {
            BinderMode::Implicit
        }
    }

    fn parse_tuple_fields(&mut self) -> Vec<Field> {
        self.expect_kind(&TokenKind::LParen, "expected `(`");
        let mut fields = Vec::new();

        while !self.at_eof() && !self.at_kind(&TokenKind::RParen) {
            if self.eat_kind(&TokenKind::Comma).is_some() {
                continue;
            }
            let start = self.current_span().start;
            let ty = self.parse_type(&[Stop::Comma, Stop::RParen]);
            fields.push(Field {
                attributes: Vec::new(),
                public: false,
                name: None,
                span: self.span(start, ty.span.end),
                ty,
            });

            if self.eat_kind(&TokenKind::Comma).is_none() {
                break;
            }
        }

        self.expect_kind(&TokenKind::RParen, "expected `)` after tuple fields");
        fields
    }

    fn parse_named_fields(&mut self) -> Vec<Field> {
        self.expect_kind(&TokenKind::LBrace, "expected `{`");
        let mut fields = Vec::new();

        while !self.at_eof() && !self.at_kind(&TokenKind::RBrace) {
            if self.eat_kind(&TokenKind::Comma).is_some() {
                continue;
            }

            let attributes = self.parse_attributes();
            let public = self.eat_keyword(Keyword::Pub).is_some();
            let start = self.current_span().start;
            let name = self.expect_ident("expected field name");
            self.expect_kind(&TokenKind::Colon, "expected `:` after field name");
            let ty = self.parse_type(&[Stop::Comma, Stop::RBrace]);
            fields.push(Field {
                attributes,
                public,
                name,
                span: self.span(start, ty.span.end),
                ty,
            });

            if self.eat_kind(&TokenKind::Comma).is_none() {
                break;
            }
        }

        self.expect_kind(&TokenKind::RBrace, "expected `}` after fields");
        fields
    }

    fn parse_where_clauses(&mut self, stops: &[Stop]) -> Vec<Expr> {
        let mut clauses = Vec::new();
        while self.eat_keyword(Keyword::Where).is_some() {
            clauses.push(self.parse_expr(stops));
            self.eat_kind(&TokenKind::Comma);
        }
        clauses
    }

    fn parse_contracts(&mut self, keyword: Keyword) -> Vec<Expr> {
        let mut clauses = Vec::new();
        while self.eat_keyword(keyword).is_some() {
            clauses.push(self.parse_expr(&[
                Stop::Keyword(Keyword::Requires),
                Stop::Keyword(Keyword::Ensures),
                Stop::LBrace,
                Stop::Semicolon,
            ]));
        }
        clauses
    }

    fn parse_type(&mut self, stops: &[Stop]) -> TypeExpr {
        self.parse_type_bp(0, stops)
    }

    fn parse_type_bp(&mut self, min_bp: u8, stops: &[Stop]) -> TypeExpr {
        let mut left = self.parse_type_primary(stops);

        loop {
            if self.at_eof() || self.at_stop(stops) {
                break;
            }

            if self.at_kind(&TokenKind::Lt) {
                let start = left.span.start;
                let args = self.parse_type_args();
                left = TypeExpr::new(
                    TypeExprKind::Generic {
                        base: Box::new(left),
                        args,
                    },
                    self.span(start, self.previous_end()),
                );
                continue;
            }

            if self.at_kind(&TokenKind::LParen) {
                let start = left.span.start;
                let args = self.parse_type_call_args();
                left = TypeExpr::new(
                    TypeExprKind::Call {
                        callee: Box::new(left),
                        args,
                    },
                    self.span(start, self.previous_end()),
                );
                continue;
            }

            let Some((op, left_bp, right_bp)) = self.current_type_binary_op() else {
                break;
            };
            if left_bp < min_bp {
                break;
            }
            self.bump();
            let right = self.parse_type_bp(right_bp, stops);
            let span = self.span(left.span.start, right.span.end);
            left = TypeExpr::new(
                TypeExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }

        left
    }

    fn parse_type_primary(&mut self, stops: &[Stop]) -> TypeExpr {
        if self.at_stop(stops) {
            let span = self.current_span();
            self.error_here("AVP0002", "expected type");
            return TypeExpr::new(TypeExprKind::Unknown, span);
        }

        let token = self.current().clone();
        match token.kind {
            TokenKind::Ident(_) => {
                let path = self.parse_path_colon();
                TypeExpr::new(TypeExprKind::Path(path.clone()), path.span)
            }
            TokenKind::HoleIdent(name) => {
                self.bump();
                TypeExpr::new(TypeExprKind::Hole(name), token.span)
            }
            TokenKind::Underscore => {
                self.bump();
                TypeExpr::new(TypeExprKind::Hole("_".to_string()), token.span)
            }
            TokenKind::LParen if self.at_parenthesized_type_binder_start() => {
                self.parse_parenthesized_pi_type(stops)
            }
            TokenKind::LParen => self.parse_tuple_type(),
            TokenKind::LBrace if self.at_braced_param_start() => self.parse_braced_pi_type(stops),
            TokenKind::LBracket => self.parse_array_or_slice_type(),
            _ => {
                self.error_here("AVP0002", "expected type");
                self.bump();
                TypeExpr::new(TypeExprKind::Unknown, token.span)
            }
        }
    }

    fn parse_parenthesized_pi_type(&mut self, stops: &[Stop]) -> TypeExpr {
        let start = self.expect_kind(&TokenKind::LParen, "expected `(`").start;
        let mut param = self.parse_explicit_param(&[Stop::RParen]);
        let param_end = self
            .expect_kind(&TokenKind::RParen, "expected `)` after dependent binder")
            .end;
        param.span = self.span(start, param_end);
        self.parse_pi_type(start, param, stops)
    }

    fn parse_braced_pi_type(&mut self, stops: &[Stop]) -> TypeExpr {
        let start = self.current_span().start;
        let param = self.parse_braced_param();
        self.parse_pi_type(start, param, stops)
    }

    fn parse_pi_type(&mut self, start: usize, param: Param, stops: &[Stop]) -> TypeExpr {
        self.expect_kind(&TokenKind::Arrow, "expected `->` after dependent binder");
        let body = self.parse_type(stops);
        TypeExpr::new(
            TypeExprKind::Pi {
                param: Box::new(param),
                body: Box::new(body.clone()),
            },
            self.span(start, body.span.end),
        )
    }

    fn parse_tuple_type(&mut self) -> TypeExpr {
        let start = self.expect_kind(&TokenKind::LParen, "expected `(`").start;
        if self.at_kind(&TokenKind::RParen) {
            let end = self.bump().end;
            return TypeExpr::new(TypeExprKind::Tuple(Vec::new()), self.span(start, end));
        }

        let mut elements = Vec::new();
        elements.push(self.parse_type(&[Stop::Comma, Stop::RParen]));
        let is_tuple = self.eat_kind(&TokenKind::Comma).is_some();
        if is_tuple {
            while !self.at_eof() && !self.at_kind(&TokenKind::RParen) {
                if self.eat_kind(&TokenKind::Comma).is_some() {
                    continue;
                }
                elements.push(self.parse_type(&[Stop::Comma, Stop::RParen]));
                self.eat_kind(&TokenKind::Comma);
            }
        }
        let end = self
            .expect_kind(&TokenKind::RParen, "expected `)` after tuple type")
            .end;

        if is_tuple {
            TypeExpr::new(TypeExprKind::Tuple(elements), self.span(start, end))
        } else {
            let mut only = elements
                .pop()
                .unwrap_or_else(|| TypeExpr::new(TypeExprKind::Unknown, self.span(start, end)));
            only.span = self.span(start, end);
            only
        }
    }

    fn parse_array_or_slice_type(&mut self) -> TypeExpr {
        let start = self.expect_kind(&TokenKind::LBracket, "expected `[`").start;
        let element = self.parse_type(&[Stop::Semicolon, Stop::RBracket]);
        if self.eat_kind(&TokenKind::Semicolon).is_some() {
            let len = self.parse_expr(&[Stop::RBracket]);
            let end = self
                .expect_kind(&TokenKind::RBracket, "expected `]` after array type")
                .end;
            TypeExpr::new(
                TypeExprKind::Array {
                    element: Box::new(element),
                    len: Box::new(len),
                },
                self.span(start, end),
            )
        } else {
            let end = self
                .expect_kind(&TokenKind::RBracket, "expected `]` after slice type")
                .end;
            TypeExpr::new(
                TypeExprKind::Slice(Box::new(element)),
                self.span(start, end),
            )
        }
    }

    fn parse_type_args(&mut self) -> Vec<TypeExpr> {
        self.expect_kind(&TokenKind::Lt, "expected `<`");
        let mut args = Vec::new();
        while !self.at_eof() && !self.at_kind(&TokenKind::Gt) {
            if self.eat_kind(&TokenKind::Comma).is_some() {
                continue;
            }
            args.push(self.parse_type(&[Stop::Comma, Stop::Gt]));
            if self.eat_kind(&TokenKind::Comma).is_none() {
                break;
            }
        }
        self.expect_kind(&TokenKind::Gt, "expected `>` after type arguments");
        args
    }

    fn parse_type_call_args(&mut self) -> Vec<TypeExpr> {
        self.expect_kind(&TokenKind::LParen, "expected `(`");
        let mut args = Vec::new();
        while !self.at_eof() && !self.at_kind(&TokenKind::RParen) {
            if self.eat_kind(&TokenKind::Comma).is_some() {
                continue;
            }
            args.push(self.parse_type(&[Stop::Comma, Stop::RParen]));
            if self.eat_kind(&TokenKind::Comma).is_none() {
                break;
            }
        }
        self.expect_kind(&TokenKind::RParen, "expected `)` after type call");
        args
    }

    fn parse_expr(&mut self, stops: &[Stop]) -> Expr {
        self.parse_expr_bp(0, stops)
    }

    fn parse_expr_bp(&mut self, min_bp: u8, stops: &[Stop]) -> Expr {
        let mut left = self.parse_expr_primary(stops);

        loop {
            if self.at_eof() || self.at_stop(stops) {
                break;
            }

            if self.at_kind(&TokenKind::LParen) {
                let start = left.span.start;
                let args = self.parse_call_args();
                left = Expr::new(
                    ExprKind::Call {
                        callee: Box::new(left),
                        args,
                    },
                    self.span(start, self.previous_end()),
                );
                continue;
            }

            if self.eat_kind(&TokenKind::Dot).is_some() {
                let start = left.span.start;
                let name = self.expect_ident("expected field name").unwrap_or_default();
                left = Expr::new(
                    ExprKind::Field {
                        base: Box::new(left),
                        name,
                    },
                    self.span(start, self.previous_end()),
                );
                continue;
            }

            if self.eat_kind(&TokenKind::LBracket).is_some() {
                let start = left.span.start;
                let index = self.parse_expr(&[Stop::RBracket]);
                let end = self
                    .expect_kind(&TokenKind::RBracket, "expected `]` after index")
                    .end;
                left = Expr::new(
                    ExprKind::Index {
                        base: Box::new(left),
                        index: Box::new(index),
                    },
                    self.span(start, end),
                );
                continue;
            }

            let Some((op, left_bp, right_bp)) = self.current_expr_binary_op() else {
                break;
            };
            if left_bp < min_bp {
                break;
            }
            self.bump();
            let right = self.parse_expr_bp(right_bp, stops);
            let span = self.span(left.span.start, right.span.end);
            left = Expr::new(
                ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }

        left
    }

    fn parse_expr_primary(&mut self, stops: &[Stop]) -> Expr {
        if self.at_stop(stops) {
            let span = self.current_span();
            self.error_here("AVP0003", "expected expression");
            return Expr::new(ExprKind::Unknown, span);
        }

        let token = self.current().clone();
        match token.kind {
            TokenKind::Ident(_) => {
                let path = self.parse_path_colon();
                Expr::new(ExprKind::Path(path.clone()), path.span)
            }
            TokenKind::Keyword(Keyword::True) => {
                self.bump();
                Expr::new(ExprKind::Bool(true), token.span)
            }
            TokenKind::Keyword(Keyword::False) => {
                self.bump();
                Expr::new(ExprKind::Bool(false), token.span)
            }
            TokenKind::IntLiteral(value) => {
                self.bump();
                Expr::new(ExprKind::Int(value), token.span)
            }
            TokenKind::FloatLiteral(value) => {
                self.bump();
                Expr::new(ExprKind::Float(value), token.span)
            }
            TokenKind::StringLiteral(value) => {
                self.bump();
                Expr::new(ExprKind::String(value), token.span)
            }
            TokenKind::CharLiteral(value) => {
                self.bump();
                Expr::new(ExprKind::Char(value), token.span)
            }
            TokenKind::HoleIdent(name) => {
                self.bump();
                Expr::new(ExprKind::Hole(name), token.span)
            }
            TokenKind::LParen => self.parse_tuple_expr(),
            TokenKind::LBrace => {
                let block = self.parse_block();
                Expr::new(ExprKind::Block(block.clone()), block.span)
            }
            TokenKind::Keyword(Keyword::Match) => self.parse_match_expr(),
            TokenKind::Keyword(Keyword::If) => self.parse_if_expr(),
            TokenKind::Keyword(Keyword::While) => self.parse_while_expr(),
            TokenKind::Keyword(Keyword::For) => self.parse_for_expr(),
            TokenKind::Keyword(Keyword::Loop) => self.parse_loop_expr(),
            TokenKind::Keyword(Keyword::Rewrite) => self.parse_rewrite_expr(stops),
            TokenKind::Keyword(Keyword::Impossible) => {
                self.bump();
                Expr::new(ExprKind::Impossible, token.span)
            }
            TokenKind::Keyword(Keyword::Return) => self.parse_return_expr(stops),
            TokenKind::Keyword(Keyword::Break) => self.parse_break_expr(stops),
            TokenKind::Keyword(Keyword::Continue) => {
                self.bump();
                Expr::new(ExprKind::Continue, token.span)
            }
            _ => {
                self.error_here("AVP0003", "expected expression");
                self.bump();
                Expr::new(ExprKind::Unknown, token.span)
            }
        }
    }

    fn parse_tuple_expr(&mut self) -> Expr {
        let start = self.expect_kind(&TokenKind::LParen, "expected `(`").start;
        if self.at_kind(&TokenKind::RParen) {
            let end = self.bump().end;
            return Expr::new(ExprKind::Tuple(Vec::new()), self.span(start, end));
        }

        let mut elements = Vec::new();
        elements.push(self.parse_expr(&[Stop::Comma, Stop::RParen]));
        let is_tuple = self.eat_kind(&TokenKind::Comma).is_some();
        if is_tuple {
            while !self.at_eof() && !self.at_kind(&TokenKind::RParen) {
                if self.eat_kind(&TokenKind::Comma).is_some() {
                    continue;
                }
                elements.push(self.parse_expr(&[Stop::Comma, Stop::RParen]));
                self.eat_kind(&TokenKind::Comma);
            }
        }
        let end = self
            .expect_kind(&TokenKind::RParen, "expected `)` after tuple expression")
            .end;

        if is_tuple {
            Expr::new(ExprKind::Tuple(elements), self.span(start, end))
        } else {
            let mut only = elements
                .pop()
                .unwrap_or_else(|| Expr::new(ExprKind::Unknown, self.span(start, end)));
            only.span = self.span(start, end);
            only
        }
    }

    fn parse_call_args(&mut self) -> Vec<Expr> {
        self.expect_kind(&TokenKind::LParen, "expected `(`");
        let mut args = Vec::new();
        while !self.at_eof() && !self.at_kind(&TokenKind::RParen) {
            if self.eat_kind(&TokenKind::Comma).is_some() {
                continue;
            }
            args.push(self.parse_expr(&[Stop::Comma, Stop::RParen]));
            if self.eat_kind(&TokenKind::Comma).is_none() {
                break;
            }
        }
        self.expect_kind(&TokenKind::RParen, "expected `)` after call");
        args
    }

    fn parse_match_expr(&mut self) -> Expr {
        let start = self
            .expect_keyword(Keyword::Match, "expected `match`")
            .start;
        let scrutinee = self.parse_expr(&[Stop::LBrace]);
        self.expect_kind(&TokenKind::LBrace, "expected `{` after match scrutinee");

        let mut arms = Vec::new();
        while !self.at_eof() && !self.at_kind(&TokenKind::RBrace) {
            if self.eat_kind(&TokenKind::Comma).is_some() {
                continue;
            }
            let arm_start = self.current_span().start;
            let pattern = self.parse_pattern();
            self.expect_kind(&TokenKind::FatArrow, "expected `=>` in match arm");
            let body = self.parse_expr(&[Stop::Comma, Stop::RBrace]);
            let span = self.span(arm_start, body.span.end);
            arms.push(MatchArm {
                pattern,
                body,
                span,
            });
            self.eat_kind(&TokenKind::Comma);
        }
        let end = self
            .expect_kind(&TokenKind::RBrace, "expected `}` after match arms")
            .end;

        Expr::new(
            ExprKind::Match {
                scrutinee: Box::new(scrutinee),
                arms,
            },
            self.span(start, end),
        )
    }

    fn parse_if_expr(&mut self) -> Expr {
        let start = self.expect_keyword(Keyword::If, "expected `if`").start;
        let condition = self.parse_expr(&[Stop::LBrace]);
        let then_branch = self.parse_block();
        let else_branch = if self.eat_keyword(Keyword::Else).is_some() {
            if self.at_keyword(Keyword::If) {
                Some(Box::new(self.parse_if_expr()))
            } else if self.at_kind(&TokenKind::LBrace) {
                let block = self.parse_block();
                Some(Box::new(Expr::new(
                    ExprKind::Block(block.clone()),
                    block.span,
                )))
            } else {
                Some(Box::new(self.parse_expr(&[
                    Stop::Comma,
                    Stop::Semicolon,
                    Stop::RBrace,
                ])))
            }
        } else {
            None
        };
        let end = else_branch
            .as_ref()
            .map(|expr| expr.span.end)
            .unwrap_or(then_branch.span.end);

        Expr::new(
            ExprKind::If {
                condition: Box::new(condition),
                then_branch,
                else_branch,
            },
            self.span(start, end),
        )
    }

    fn parse_while_expr(&mut self) -> Expr {
        let start = self
            .expect_keyword(Keyword::While, "expected `while`")
            .start;
        let condition = self.parse_expr(&[Stop::LBrace]);
        let body = self.parse_block();
        let span = self.span(start, body.span.end);
        Expr::new(
            ExprKind::While {
                condition: Box::new(condition),
                body,
            },
            span,
        )
    }

    fn parse_for_expr(&mut self) -> Expr {
        let start = self.expect_keyword(Keyword::For, "expected `for`").start;
        let pattern = self.parse_pattern();
        self.expect_keyword(Keyword::In, "expected `in` in for expression");
        let iterable = self.parse_expr(&[Stop::LBrace]);
        let body = self.parse_block();
        let span = self.span(start, body.span.end);
        Expr::new(
            ExprKind::For {
                pattern,
                iterable: Box::new(iterable),
                body,
            },
            span,
        )
    }

    fn parse_loop_expr(&mut self) -> Expr {
        let start = self.expect_keyword(Keyword::Loop, "expected `loop`").start;
        let body = self.parse_block();
        let span = self.span(start, body.span.end);
        Expr::new(ExprKind::Loop { body }, span)
    }

    fn parse_rewrite_expr(&mut self, stops: &[Stop]) -> Expr {
        let start = self
            .expect_keyword(Keyword::Rewrite, "expected `rewrite`")
            .start;
        let proof = self.parse_expr(&[Stop::Keyword(Keyword::In)]);
        self.expect_keyword(Keyword::In, "expected `in` after rewrite proof");
        let body = self.parse_expr(stops);
        let span = self.span(start, body.span.end);
        Expr::new(
            ExprKind::Rewrite {
                proof: Box::new(proof),
                body: Box::new(body),
            },
            span,
        )
    }

    fn parse_break_expr(&mut self, stops: &[Stop]) -> Expr {
        let start = self
            .expect_keyword(Keyword::Break, "expected `break`")
            .start;
        if self.at_stop(stops) || self.at_kind(&TokenKind::Semicolon) {
            return Expr::new(ExprKind::Break(None), self.span(start, self.previous_end()));
        }
        let value = self.parse_expr(stops);
        Expr::new(
            ExprKind::Break(Some(Box::new(value.clone()))),
            self.span(start, value.span.end),
        )
    }

    fn parse_return_expr(&mut self, stops: &[Stop]) -> Expr {
        let start = self
            .expect_keyword(Keyword::Return, "expected `return`")
            .start;
        if self.at_stop(stops) || self.at_kind(&TokenKind::Semicolon) {
            return Expr::new(
                ExprKind::Return(None),
                self.span(start, self.previous_end()),
            );
        }
        let value = self.parse_expr(stops);
        Expr::new(
            ExprKind::Return(Some(Box::new(value.clone()))),
            self.span(start, value.span.end),
        )
    }

    fn parse_block(&mut self) -> Block {
        let start = self.expect_kind(&TokenKind::LBrace, "expected `{`").start;
        let mut statements = Vec::new();

        while !self.at_eof() && !self.at_kind(&TokenKind::RBrace) {
            if self.at_keyword(Keyword::Let) {
                statements.push(self.parse_let_stmt());
            } else {
                let expr = self.parse_expr(&[Stop::Semicolon, Stop::RBrace]);
                self.eat_kind(&TokenKind::Semicolon);
                statements.push(Stmt::Expr(expr));
            }
        }

        let end = self
            .expect_kind(&TokenKind::RBrace, "expected `}` after block")
            .end;
        Block {
            statements,
            span: self.span(start, end),
        }
    }

    fn parse_let_stmt(&mut self) -> Stmt {
        let start = self.expect_keyword(Keyword::Let, "expected `let`").start;
        let pattern = self.parse_pattern();
        let ty = if self.eat_kind(&TokenKind::Colon).is_some() {
            Some(self.parse_type(&[Stop::Eq, Stop::Semicolon]))
        } else {
            None
        };
        let value = if self.eat_kind(&TokenKind::Eq).is_some() {
            Some(self.parse_expr(&[Stop::Semicolon]))
        } else {
            None
        };
        let end = self
            .expect_kind(&TokenKind::Semicolon, "expected `;` after let statement")
            .end;

        Stmt::Let {
            pattern,
            ty,
            value,
            span: self.span(start, end),
        }
    }

    fn parse_pattern(&mut self) -> Pattern {
        let token = self.current().clone();
        match token.kind {
            TokenKind::Underscore => {
                self.bump();
                Pattern::new(PatternKind::Wildcard, token.span)
            }
            TokenKind::IntLiteral(value) => {
                self.bump();
                Pattern::new(PatternKind::Int(value), token.span)
            }
            TokenKind::Ident(_) => {
                let path = self.parse_path_colon();
                if self.at_kind(&TokenKind::LBrace) {
                    self.parse_struct_pattern(path)
                } else if self.at_kind(&TokenKind::LParen) {
                    self.parse_tuple_pattern(Some(path))
                } else if path.segments.len() == 1 && starts_lowercase(&path.segments[0]) {
                    let span = path.span;
                    Pattern::new(PatternKind::Binding(path.segments[0].clone()), span)
                } else {
                    Pattern::new(PatternKind::Path(path.clone()), path.span)
                }
            }
            TokenKind::LParen => self.parse_tuple_pattern(None),
            _ => {
                self.error_here("AVP0004", "expected pattern");
                self.bump();
                Pattern::new(PatternKind::Unknown, token.span)
            }
        }
    }

    fn parse_tuple_pattern(&mut self, path: Option<Path>) -> Pattern {
        let start = path
            .as_ref()
            .map(|path| path.span.start)
            .unwrap_or_else(|| self.current_span().start);
        self.expect_kind(&TokenKind::LParen, "expected `(` in tuple pattern");
        let mut elements = Vec::new();

        while !self.at_eof() && !self.at_kind(&TokenKind::RParen) {
            if self.eat_kind(&TokenKind::Comma).is_some() {
                continue;
            }
            elements.push(self.parse_pattern());
            if self.eat_kind(&TokenKind::Comma).is_none() {
                break;
            }
        }

        let end = self
            .expect_kind(&TokenKind::RParen, "expected `)` after tuple pattern")
            .end;
        Pattern::new(PatternKind::Tuple { path, elements }, self.span(start, end))
    }

    fn parse_struct_pattern(&mut self, path: Path) -> Pattern {
        let start = path.span.start;
        self.expect_kind(&TokenKind::LBrace, "expected `{` in struct pattern");
        let mut fields = Vec::new();
        let mut rest = false;

        while !self.at_eof() && !self.at_kind(&TokenKind::RBrace) {
            if self.eat_kind(&TokenKind::Comma).is_some() {
                continue;
            }
            if self.eat_kind(&TokenKind::DotDot).is_some() {
                rest = true;
                self.eat_kind(&TokenKind::Comma);
                continue;
            }

            let field_start = self.current_span().start;
            let name = self
                .expect_ident("expected pattern field")
                .unwrap_or_default();
            let pattern = if self.eat_kind(&TokenKind::Colon).is_some() {
                Some(self.parse_pattern())
            } else {
                None
            };
            fields.push(PatternField {
                name,
                pattern,
                span: self.span(field_start, self.previous_end()),
            });
            self.eat_kind(&TokenKind::Comma);
        }

        let end = self
            .expect_kind(&TokenKind::RBrace, "expected `}` after struct pattern")
            .end;
        Pattern::new(
            PatternKind::Struct { path, fields, rest },
            self.span(start, end),
        )
    }

    fn parse_package_path(&mut self) -> Path {
        let start = self.current_span().start;
        let mut segments = Vec::new();
        segments.push(
            self.expect_ident("expected path segment")
                .unwrap_or_default(),
        );
        while self.eat_kind(&TokenKind::Dot).is_some() {
            segments.push(
                self.expect_ident("expected path segment")
                    .unwrap_or_default(),
            );
        }
        Path::new(segments, self.span(start, self.previous_end()))
    }

    fn parse_path_colon(&mut self) -> Path {
        let start = self.current_span().start;
        let mut segments = Vec::new();
        segments.push(
            self.expect_ident("expected path segment")
                .unwrap_or_default(),
        );
        while self.eat_kind(&TokenKind::DoubleColon).is_some() {
            segments.push(
                self.expect_ident("expected path segment")
                    .unwrap_or_default(),
            );
        }
        Path::new(segments, self.span(start, self.previous_end()))
    }

    fn parse_fn_flavor(&mut self) -> FnFlavor {
        if self.eat_keyword(Keyword::Proof).is_some() {
            FnFlavor::Proof
        } else if self.eat_keyword(Keyword::Total).is_some() {
            FnFlavor::Total
        } else if self.eat_keyword(Keyword::Partial).is_some() {
            FnFlavor::Partial
        } else {
            FnFlavor::Plain
        }
    }

    fn current_expr_binary_op(&self) -> Option<(BinaryOp, u8, u8)> {
        match self.current().kind {
            TokenKind::PipePipe => Some((BinaryOp::Or, 1, 2)),
            TokenKind::AmpAmp => Some((BinaryOp::And, 3, 4)),
            TokenKind::EqEq => Some((BinaryOp::Eq, 5, 6)),
            TokenKind::BangEq => Some((BinaryOp::NotEq, 5, 6)),
            TokenKind::Lt => Some((BinaryOp::Lt, 7, 8)),
            TokenKind::LtEq => Some((BinaryOp::LtEq, 7, 8)),
            TokenKind::Gt => Some((BinaryOp::Gt, 7, 8)),
            TokenKind::GtEq => Some((BinaryOp::GtEq, 7, 8)),
            TokenKind::Plus => Some((BinaryOp::Add, 9, 10)),
            TokenKind::Minus => Some((BinaryOp::Sub, 9, 10)),
            TokenKind::Star => Some((BinaryOp::Mul, 11, 12)),
            TokenKind::Slash => Some((BinaryOp::Div, 11, 12)),
            TokenKind::Percent => Some((BinaryOp::Rem, 11, 12)),
            _ => None,
        }
    }

    fn current_type_binary_op(&self) -> Option<(BinaryOp, u8, u8)> {
        match self.current().kind {
            TokenKind::EqEq => Some((BinaryOp::Eq, 1, 2)),
            TokenKind::BangEq => Some((BinaryOp::NotEq, 1, 2)),
            TokenKind::Plus => Some((BinaryOp::Add, 3, 4)),
            TokenKind::Minus => Some((BinaryOp::Sub, 3, 4)),
            TokenKind::Star => Some((BinaryOp::Mul, 5, 6)),
            TokenKind::Slash => Some((BinaryOp::Div, 5, 6)),
            TokenKind::Percent => Some((BinaryOp::Rem, 5, 6)),
            _ => None,
        }
    }

    fn parse_attributes(&mut self) -> Vec<Attribute> {
        let mut attributes = Vec::new();

        while self.at_kind(&TokenKind::Hash) {
            let start = self.bump().start;
            if self.eat_kind(&TokenKind::LBracket).is_some() {
                let path = self.parse_path_colon();
                let args = if self.at_kind(&TokenKind::LParen) {
                    self.parse_attribute_args()
                } else {
                    Vec::new()
                };
                let end = self
                    .expect_kind(&TokenKind::RBracket, "expected `]` after attribute")
                    .end;
                attributes.push(Attribute {
                    path,
                    args,
                    span: self.span(start, end),
                });
            } else {
                self.error_here("AVP0007", "expected `[` after `#`");
            }
        }

        attributes
    }

    fn parse_attribute_args(&mut self) -> Vec<AttributeArg> {
        self.expect_kind(&TokenKind::LParen, "expected `(` in attribute");
        let mut args = Vec::new();

        while !self.at_eof() && !self.at_kind(&TokenKind::RParen) {
            if self.eat_kind(&TokenKind::Comma).is_some() {
                continue;
            }

            if matches!(self.current().kind, TokenKind::Ident(_))
                && self.peek_kind(1, &TokenKind::Eq)
            {
                let start = self.current_span().start;
                let name = self
                    .expect_ident("expected attribute argument name")
                    .unwrap_or_default();
                self.expect_kind(&TokenKind::Eq, "expected `=` in attribute argument");
                let value = self.parse_expr(&[Stop::Comma, Stop::RParen]);
                args.push(AttributeArg::NameValue {
                    name,
                    span: self.span(start, value.span.end),
                    value,
                });
            } else {
                args.push(AttributeArg::Expr(
                    self.parse_expr(&[Stop::Comma, Stop::RParen]),
                ));
            }

            if self.eat_kind(&TokenKind::Comma).is_none() {
                break;
            }
        }

        self.expect_kind(&TokenKind::RParen, "expected `)` after attribute arguments");
        args
    }

    fn peek_kind(&self, offset: usize, kind: &TokenKind) -> bool {
        self.tokens
            .get(self.pos + offset)
            .is_some_and(|token| token_kind_eq(&token.kind, kind))
    }

    fn at_braced_param_start(&self) -> bool {
        if !self.at_kind(&TokenKind::LBrace) {
            return false;
        }

        match self.tokens.get(self.pos + 1).map(|token| &token.kind) {
            Some(TokenKind::Keyword(Keyword::Auto | Keyword::Erased | Keyword::Implicit)) => true,
            Some(TokenKind::Ident(_) | TokenKind::Underscore) => {
                self.peek_kind(2, &TokenKind::Colon)
            }
            _ => false,
        }
    }

    fn at_parenthesized_type_binder_start(&self) -> bool {
        if !self.at_kind(&TokenKind::LParen) {
            return false;
        }

        matches!(
            self.tokens.get(self.pos + 1).map(|token| &token.kind),
            Some(TokenKind::Ident(_) | TokenKind::Underscore)
        ) && self.peek_kind(2, &TokenKind::Colon)
    }

    fn synchronize_item(&mut self) {
        while !self.at_eof() {
            if matches!(
                self.current().kind,
                TokenKind::Keyword(Keyword::Import)
                    | TokenKind::Keyword(Keyword::Enum)
                    | TokenKind::Keyword(Keyword::Struct)
                    | TokenKind::Keyword(Keyword::Type)
                    | TokenKind::Keyword(Keyword::Fn)
                    | TokenKind::Keyword(Keyword::Proof)
                    | TokenKind::Keyword(Keyword::Impl)
            ) {
                break;
            }
            self.bump();
        }
    }

    fn at_stop(&self, stops: &[Stop]) -> bool {
        stops.iter().any(|stop| match stop {
            Stop::Comma => self.at_kind(&TokenKind::Comma),
            Stop::Eq => self.at_kind(&TokenKind::Eq),
            Stop::LBrace => self.at_kind(&TokenKind::LBrace),
            Stop::RParen => self.at_kind(&TokenKind::RParen),
            Stop::RBrace => self.at_kind(&TokenKind::RBrace),
            Stop::RBracket => self.at_kind(&TokenKind::RBracket),
            Stop::Gt => self.at_kind(&TokenKind::Gt),
            Stop::Semicolon => self.at_kind(&TokenKind::Semicolon),
            Stop::Keyword(keyword) => self.at_keyword(*keyword),
        })
    }

    fn at_eof(&self) -> bool {
        matches!(self.current().kind, TokenKind::Eof)
    }

    fn at_kind(&self, kind: &TokenKind) -> bool {
        token_kind_eq(&self.current().kind, kind)
    }

    fn at_keyword(&self, keyword: Keyword) -> bool {
        matches!(self.current().kind, TokenKind::Keyword(found) if found == keyword)
    }

    fn eat_kind(&mut self, kind: &TokenKind) -> Option<Span> {
        if self.at_kind(kind) {
            Some(self.bump())
        } else {
            None
        }
    }

    fn eat_keyword(&mut self, keyword: Keyword) -> Option<Span> {
        if self.at_keyword(keyword) {
            Some(self.bump())
        } else {
            None
        }
    }

    fn expect_kind(&mut self, kind: &TokenKind, message: &'static str) -> Span {
        if self.at_kind(kind) {
            self.bump()
        } else {
            self.error_here("AVP0005", message);
            self.current_span()
        }
    }

    fn expect_keyword(&mut self, keyword: Keyword, message: &'static str) -> Span {
        if self.at_keyword(keyword) {
            self.bump()
        } else {
            self.error_here("AVP0005", message);
            self.current_span()
        }
    }

    fn expect_ident(&mut self, message: &'static str) -> Option<String> {
        match self.current().kind.clone() {
            TokenKind::Ident(value) => {
                self.bump();
                Some(value)
            }
            _ => {
                self.error_here("AVP0006", message);
                None
            }
        }
    }

    fn error_here(&mut self, code: &'static str, message: impl Into<String>) {
        let span = self.current_span();
        self.diagnostics
            .push(Diagnostic::error(code, message).with_span(span));
    }

    fn current(&self) -> &Token {
        self.tokens
            .get(self.pos)
            .or_else(|| self.tokens.last())
            .expect("parser requires at least EOF token")
    }

    fn current_span(&self) -> Span {
        self.current().span
    }

    fn bump(&mut self) -> Span {
        let span = self.current_span();
        if !self.at_eof() {
            self.pos += 1;
        }
        span
    }

    fn previous_end(&self) -> usize {
        self.pos
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .map(|token| token.span.end)
            .unwrap_or_else(|| self.current_span().start)
    }

    fn span(&self, start: usize, end: usize) -> Span {
        let file_id = self
            .tokens
            .first()
            .map(|token| token.span.file_id)
            .unwrap_or(FileId(0));
        Span::new(file_id, start, end.max(start))
    }
}

fn is_trivia(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::LineComment(_)
            | TokenKind::DocLineComment(_)
            | TokenKind::BlockComment(_)
            | TokenKind::DocBlockComment(_)
    )
}

fn token_kind_eq(left: &TokenKind, right: &TokenKind) -> bool {
    std::mem::discriminant(left) == std::mem::discriminant(right)
}

fn starts_lowercase(value: &str) -> bool {
    value
        .chars()
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_lowercase())
}

#[cfg(test)]
mod tests {
    use crate::lexer;
    use crate::source::{FileId, SourceFile};

    use super::*;

    fn parse_text(text: &str) -> ParseResult {
        let file = SourceFile::new(FileId(0), "test.avtn", text);
        let lexed = lexer::lex(&file);
        assert!(lexed.diagnostics.is_empty());
        parse_tokens(&lexed.tokens)
    }

    #[test]
    fn parses_dependent_vect_enum() {
        let result = parse_text(
            r#"
package examples.nat_vect

enum Nat {
    Z,
    S(Nat),
}

enum Vect<T, const N: Nat> {
    Nil
        where N == Z,

    Cons<const M: Nat> {
        head: T,
        tail: Vect<T, M>,
    }
        where N == S(M),
}
"#,
        );

        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(
            result.module.package.as_ref().unwrap().segments,
            ["examples", "nat_vect"]
        );
        assert_eq!(result.module.items.len(), 2);

        let Item::Enum(vect) = &result.module.items[1] else {
            panic!("expected Vect enum");
        };
        assert_eq!(vect.name, "Vect");
        assert_eq!(vect.generics.len(), 2);
        assert_eq!(vect.variants.len(), 2);
        assert_eq!(vect.variants[0].where_clauses.len(), 1);
        assert_eq!(vect.variants[1].where_clauses.len(), 1);
    }

    #[test]
    fn parses_match_and_rewrite_functions() {
        let result = parse_text(
            r#"
fn head<T, const N: Nat>(xs: Vect<T, S(N)>) -> T {
    match xs {
        Vect::Cons { head, .. } => head,
    }
}

proof fn plus_zero_right(n: Nat) -> n + Z == n {
    match n {
        Z => Refl,
        S(k) => rewrite plus_zero_right(k) in Refl,
    }
}
"#,
        );

        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(result.module.items.len(), 2);

        let Item::Fn(head) = &result.module.items[0] else {
            panic!("expected head function");
        };
        assert_eq!(head.name, "head");
        assert_eq!(head.generics.len(), 2);
        assert!(head.body.is_some());

        let Item::Fn(proof) = &result.module.items[1] else {
            panic!("expected proof function");
        };
        assert_eq!(proof.flavor, FnFlavor::Proof);
        assert_eq!(proof.name, "plus_zero_right");
    }

    #[test]
    fn parses_nat_vect_example_file() {
        let result = parse_text(include_str!("../../examples/nat_vect.avtn"));

        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(result.module.items.len(), 4);
    }

    #[test]
    fn parses_attributes_import_groups_and_control_flow() {
        let result = parse_text(
            r#"
import std.sync.{TaskGroup, Chan}

#[derive(Clone, Eq)]
pub struct Handler {
    pub count: i32,
}

#[test]
fn control(xs: Vec<i32>) -> i32 {
    let acc: i32 = 0;

    if true {
        return 1;
    } else {
        loop {
            break 2;
        }
    }

    for item in xs {
        continue;
    }

    while false {
        break;
    }

    acc
}
"#,
        );

        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(result.module.items.len(), 3);

        let Item::Import(import) = &result.module.items[0] else {
            panic!("expected import");
        };
        assert_eq!(import.path.segments, ["std", "sync"]);
        assert_eq!(import.members, ["TaskGroup", "Chan"]);

        let Item::Struct(handler) = &result.module.items[1] else {
            panic!("expected struct");
        };
        assert!(handler.public);
        assert_eq!(handler.attributes.len(), 1);
        assert_eq!(handler.fields.len(), 1);

        let Item::Fn(control) = &result.module.items[2] else {
            panic!("expected function");
        };
        assert_eq!(control.attributes.len(), 1);
        let body = control.body.as_ref().expect("control body");
        assert_eq!(body.statements.len(), 5);
        assert!(matches!(
            body.statements[1],
            Stmt::Expr(Expr {
                kind: ExprKind::If { .. },
                ..
            })
        ));
        assert!(matches!(
            body.statements[2],
            Stmt::Expr(Expr {
                kind: ExprKind::For { .. },
                ..
            })
        ));
        assert!(matches!(
            body.statements[3],
            Stmt::Expr(Expr {
                kind: ExprKind::While { .. },
                ..
            })
        ));
    }

    #[test]
    fn parses_explicit_implicit_auto_and_erased_binders() {
        let result = parse_text(
            r#"
fn safe_index<T, const N: Nat>(
    xs: Vect<T, N>,
    i: Fin<N>,
    {ctx: IndexCtx<N>},
    {implicit hint: IndexHint<N>},
    {auto p: InBounds<i, N>},
    {erased same_len: Same<N, N>},
) -> T
requires p == p
{
    xs[i]
}
"#,
        );

        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);

        let Item::Fn(item) = &result.module.items[0] else {
            panic!("expected function");
        };
        let modes = item
            .params
            .iter()
            .map(|param| param.mode)
            .collect::<Vec<_>>();
        assert_eq!(
            modes,
            [
                BinderMode::Explicit,
                BinderMode::Explicit,
                BinderMode::Implicit,
                BinderMode::Implicit,
                BinderMode::Auto,
                BinderMode::Erased,
            ]
        );
        assert_eq!(item.requires.len(), 1);
    }

    #[test]
    fn parses_standalone_erased_binder_after_function_name() {
        let result = parse_text(
            r#"
proof fn reflexive {erased n: Nat} -> n == n {
    Refl
}
"#,
        );

        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);

        let Item::Fn(item) = &result.module.items[0] else {
            panic!("expected function");
        };
        assert_eq!(item.flavor, FnFlavor::Proof);
        assert_eq!(item.params.len(), 1);
        assert_eq!(item.params[0].mode, BinderMode::Erased);
    }

    #[test]
    fn parses_dependent_pi_type_binders() {
        let result = parse_text(
            r#"
type CheckedIndex<const N: Nat> =
    (i: Fin<N>) -> {auto p: InBounds<i, N>} -> {erased same: Same<N, N>} -> T;
"#,
        );

        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);

        let Item::TypeAlias(item) = &result.module.items[0] else {
            panic!("expected type alias");
        };
        let TypeExprKind::Pi { param, body } = &item.ty.kind else {
            panic!("expected explicit pi type");
        };
        assert_eq!(param.mode, BinderMode::Explicit);

        let TypeExprKind::Pi { param, body } = &body.kind else {
            panic!("expected auto pi type");
        };
        assert_eq!(param.mode, BinderMode::Auto);

        let TypeExprKind::Pi { param, .. } = &body.kind else {
            panic!("expected erased pi type");
        };
        assert_eq!(param.mode, BinderMode::Erased);
    }
}
