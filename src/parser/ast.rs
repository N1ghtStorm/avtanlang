use std::fmt;

use crate::source::Span;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    pub package: Option<Path>,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Import(ImportItem),
    Enum(EnumItem),
    Struct(StructItem),
    TypeAlias(TypeAliasItem),
    Fn(FnItem),
    Impl(ImplItem),
    Error(Span),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportItem {
    pub path: Path,
    pub alias: Option<String>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumItem {
    pub public: bool,
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumVariant {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub kind: VariantKind,
    pub where_clauses: Vec<Expr>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VariantKind {
    Unit,
    Tuple(Vec<Field>),
    Struct(Vec<Field>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructItem {
    pub public: bool,
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub fields: Vec<Field>,
    pub where_clauses: Vec<Expr>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeAliasItem {
    pub public: bool,
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub ty: TypeExpr,
    pub where_clauses: Vec<Expr>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FnItem {
    pub public: bool,
    pub flavor: FnFlavor,
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub requires: Vec<Expr>,
    pub ensures: Vec<Expr>,
    pub body: Option<Block>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FnFlavor {
    Plain,
    Proof,
    Total,
    Partial,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImplItem {
    pub generics: Vec<GenericParam>,
    pub target: TypeExpr,
    pub items: Vec<Item>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenericParam {
    pub name: String,
    pub kind: GenericParamKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GenericParamKind {
    Type,
    Const { ty: TypeExpr },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Param {
    pub name: Pattern,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub public: bool,
    pub name: Option<String>,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeExpr {
    pub kind: TypeExprKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeExprKind {
    Path(Path),
    Generic {
        base: Box<TypeExpr>,
        args: Vec<TypeExpr>,
    },
    Call {
        callee: Box<TypeExpr>,
        args: Vec<TypeExpr>,
    },
    Tuple(Vec<TypeExpr>),
    Slice(Box<TypeExpr>),
    Array {
        element: Box<TypeExpr>,
        len: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<TypeExpr>,
        right: Box<TypeExpr>,
    },
    Hole(String),
    Unknown,
}

impl TypeExpr {
    pub fn new(kind: TypeExprKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stmt {
    Let {
        pattern: Pattern,
        ty: Option<TypeExpr>,
        value: Option<Expr>,
        span: Span,
    },
    Expr(Expr),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExprKind {
    Path(Path),
    Int(String),
    Float(String),
    String(String),
    Char(String),
    Bool(bool),
    Hole(String),
    Tuple(Vec<Expr>),
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Field {
        base: Box<Expr>,
        name: String,
    },
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Block(Block),
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    Rewrite {
        proof: Box<Expr>,
        body: Box<Expr>,
    },
    Return(Option<Box<Expr>>),
    Impossible,
    Unknown,
}

impl Expr {
    pub fn new(kind: ExprKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    pub kind: PatternKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatternKind {
    Wildcard,
    Binding(String),
    Path(Path),
    Tuple {
        path: Option<Path>,
        elements: Vec<Pattern>,
    },
    Struct {
        path: Path,
        fields: Vec<PatternField>,
        rest: bool,
    },
    Int(String),
    Unknown,
}

impl Pattern {
    pub fn new(kind: PatternKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternField {
    pub name: String,
    pub pattern: Option<Pattern>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    And,
    Or,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Path {
    pub segments: Vec<String>,
    pub span: Span,
}

impl Path {
    pub fn new(segments: Vec<String>, span: Span) -> Self {
        Self { segments, span }
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, segment) in self.segments.iter().enumerate() {
            if index > 0 {
                write!(f, "::")?;
            }
            write!(f, "{segment}")?;
        }
        Ok(())
    }
}
