use crate::parser;
use crate::source::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SymbolId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BinderId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScopeId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Namespace {
    Type,
    Value,
}

impl Namespace {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::Value => "value",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    pub package: Option<Vec<String>>,
    pub imports: Vec<Import>,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Import {
    pub path: Vec<String>,
    pub members: Vec<String>,
    pub alias: Option<String>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Enum(EnumItem),
    Struct(StructItem),
    TypeAlias(TypeAliasItem),
    Fn(FnItem),
    Impl(ImplItem),
    Error(Span),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumItem {
    pub symbol: SymbolId,
    pub scope: ScopeId,
    pub public: bool,
    pub name: String,
    pub binders: Vec<Binder>,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumVariant {
    pub symbol: SymbolId,
    pub scope: ScopeId,
    pub name: String,
    pub binders: Vec<Binder>,
    pub kind: VariantKind,
    pub where_clauses: Vec<parser::Expr>,
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
    pub symbol: SymbolId,
    pub scope: ScopeId,
    pub public: bool,
    pub name: String,
    pub binders: Vec<Binder>,
    pub fields: Vec<Field>,
    pub where_clauses: Vec<parser::Expr>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeAliasItem {
    pub symbol: SymbolId,
    pub scope: ScopeId,
    pub public: bool,
    pub name: String,
    pub binders: Vec<Binder>,
    pub ty: Type,
    pub where_clauses: Vec<parser::Expr>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FnItem {
    pub symbol: SymbolId,
    pub scope: ScopeId,
    pub public: bool,
    pub flavor: FnFlavor,
    pub name: String,
    pub binders: Vec<Binder>,
    pub return_type: Option<Type>,
    pub requires: Vec<parser::Expr>,
    pub ensures: Vec<parser::Expr>,
    pub body: Option<parser::Block>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FnFlavor {
    Plain,
    Proof,
    Total,
    Partial,
}

impl From<parser::FnFlavor> for FnFlavor {
    fn from(value: parser::FnFlavor) -> Self {
        match value {
            parser::FnFlavor::Plain => Self::Plain,
            parser::FnFlavor::Proof => Self::Proof,
            parser::FnFlavor::Total => Self::Total,
            parser::FnFlavor::Partial => Self::Partial,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImplItem {
    pub scope: ScopeId,
    pub binders: Vec<Binder>,
    pub target: Type,
    pub items: Vec<Item>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binder {
    pub id: BinderId,
    pub symbol: SymbolId,
    pub mode: BinderMode,
    pub name: String,
    pub kind: BinderKind,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinderMode {
    Explicit,
    Implicit,
    Auto,
    Erased,
}

impl From<parser::BinderMode> for BinderMode {
    fn from(value: parser::BinderMode) -> Self {
        match value {
            parser::BinderMode::Explicit => Self::Explicit,
            parser::BinderMode::Implicit => Self::Implicit,
            parser::BinderMode::Auto => Self::Auto,
            parser::BinderMode::Erased => Self::Erased,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BinderKind {
    Type,
    Value { ty: Type },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub public: bool,
    pub name: Option<String>,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Type {
    pub kind: TypeKind,
    pub span: Span,
}

impl Type {
    pub fn new(kind: TypeKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeKind {
    Path(Path),
    Apply {
        callee: Box<Type>,
        args: Vec<Type>,
        style: TypeApplyStyle,
    },
    Tuple(Vec<Type>),
    Slice(Box<Type>),
    Array {
        element: Box<Type>,
        len: Box<parser::Expr>,
    },
    Pi {
        param: Box<Binder>,
        body: Box<Type>,
    },
    Binary {
        op: parser::BinaryOp,
        left: Box<Type>,
        right: Box<Type>,
    },
    Hole(String),
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeApplyStyle {
    Angle,
    Call,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Path {
    pub segments: Vec<String>,
    pub span: Span,
}
