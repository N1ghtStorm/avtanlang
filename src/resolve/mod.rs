use std::collections::HashMap;
use std::fmt::Write;

use crate::diagnostics::Diagnostic;
use crate::hir;
use crate::parser;
use crate::source::Span;

#[derive(Clone, Debug)]
pub struct ResolveResult {
    pub module: hir::Module,
    pub symbols: SymbolTable,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn resolve_module(module: &parser::Module) -> ResolveResult {
    Resolver::new().resolve_module(module)
}

pub fn dump_symbols(symbols: &SymbolTable) -> String {
    let mut output = String::new();

    for symbol in symbols.symbols() {
        let _ = writeln!(
            output,
            "{} {}\t{}",
            symbol.namespace.as_str(),
            symbol.qualified_path.join("::"),
            symbol.kind.as_str()
        );
    }

    output
}

#[derive(Clone, Debug)]
pub struct SymbolTable {
    scopes: Vec<Scope>,
    symbols: Vec<Symbol>,
    by_key: HashMap<SymbolKey, hir::SymbolId>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope {
                id: hir::ScopeId(0),
                parent: None,
                label: "<root>".to_string(),
            }],
            symbols: Vec::new(),
            by_key: HashMap::new(),
        }
    }

    pub fn root_scope(&self) -> hir::ScopeId {
        hir::ScopeId(0)
    }

    pub fn new_scope(&mut self, parent: hir::ScopeId, label: impl Into<String>) -> hir::ScopeId {
        let id = hir::ScopeId(self.scopes.len() as u32);
        self.scopes.push(Scope {
            id,
            parent: Some(parent),
            label: label.into(),
        });
        id
    }

    pub fn scopes(&self) -> &[Scope] {
        &self.scopes
    }

    pub fn symbols(&self) -> &[Symbol] {
        &self.symbols
    }

    pub fn find(
        &self,
        scope: hir::ScopeId,
        namespace: hir::Namespace,
        name: &str,
    ) -> Option<&Symbol> {
        self.by_key
            .get(&SymbolKey {
                scope,
                namespace,
                name: name.to_string(),
            })
            .and_then(|id| self.symbols.get(id.0 as usize))
    }

    fn define(
        &mut self,
        scope: hir::ScopeId,
        namespace: hir::Namespace,
        name: impl Into<String>,
        qualified_path: Vec<String>,
        kind: SymbolKind,
        public: bool,
        span: Span,
    ) -> Result<hir::SymbolId, hir::SymbolId> {
        let name = name.into();
        let key = SymbolKey {
            scope,
            namespace,
            name: name.clone(),
        };

        if let Some(existing) = self.by_key.get(&key) {
            return Err(*existing);
        }

        let id = hir::SymbolId(self.symbols.len() as u32);
        self.symbols.push(Symbol {
            id,
            scope,
            namespace,
            name,
            qualified_path,
            kind,
            public,
            span,
        });
        self.by_key.insert(key, id);
        Ok(id)
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope {
    pub id: hir::ScopeId,
    pub parent: Option<hir::ScopeId>,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol {
    pub id: hir::SymbolId,
    pub scope: hir::ScopeId,
    pub namespace: hir::Namespace,
    pub name: String,
    pub qualified_path: Vec<String>,
    pub kind: SymbolKind,
    pub public: bool,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Enum,
    Struct,
    TypeAlias,
    Function(hir::FnFlavor),
    EnumVariant,
    Binder {
        id: hir::BinderId,
        mode: hir::BinderMode,
        kind: BinderSymbolKind,
    },
}

impl SymbolKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Enum => "enum",
            Self::Struct => "struct",
            Self::TypeAlias => "type_alias",
            Self::Function(hir::FnFlavor::Plain) => "fn",
            Self::Function(hir::FnFlavor::Proof) => "proof_fn",
            Self::Function(hir::FnFlavor::Total) => "total_fn",
            Self::Function(hir::FnFlavor::Partial) => "partial_fn",
            Self::EnumVariant => "enum_variant",
            Self::Binder {
                mode: hir::BinderMode::Explicit,
                kind: BinderSymbolKind::Type,
                ..
            } => "explicit_type_binder",
            Self::Binder {
                mode: hir::BinderMode::Implicit,
                kind: BinderSymbolKind::Type,
                ..
            } => "implicit_type_binder",
            Self::Binder {
                mode: hir::BinderMode::Auto,
                kind: BinderSymbolKind::Type,
                ..
            } => "auto_type_binder",
            Self::Binder {
                mode: hir::BinderMode::Erased,
                kind: BinderSymbolKind::Type,
                ..
            } => "erased_type_binder",
            Self::Binder {
                mode: hir::BinderMode::Explicit,
                kind: BinderSymbolKind::Value,
                ..
            } => "explicit_value_binder",
            Self::Binder {
                mode: hir::BinderMode::Implicit,
                kind: BinderSymbolKind::Value,
                ..
            } => "implicit_value_binder",
            Self::Binder {
                mode: hir::BinderMode::Auto,
                kind: BinderSymbolKind::Value,
                ..
            } => "auto_value_binder",
            Self::Binder {
                mode: hir::BinderMode::Erased,
                kind: BinderSymbolKind::Value,
                ..
            } => "erased_value_binder",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinderSymbolKind {
    Type,
    Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SymbolKey {
    scope: hir::ScopeId,
    namespace: hir::Namespace,
    name: String,
}

struct Resolver {
    symbols: SymbolTable,
    diagnostics: Vec<Diagnostic>,
    next_binder: u32,
}

impl Resolver {
    fn new() -> Self {
        Self {
            symbols: SymbolTable::new(),
            diagnostics: Vec::new(),
            next_binder: 0,
        }
    }

    fn resolve_module(mut self, module: &parser::Module) -> ResolveResult {
        let package = module.package.as_ref().map(|path| path.segments.clone());
        let parent_path = package.clone().unwrap_or_default();

        let mut imports = Vec::new();
        let mut items = Vec::new();
        let root_scope = self.symbols.root_scope();

        for item in &module.items {
            match item {
                parser::Item::Import(item) => imports.push(hir::Import {
                    path: item.path.segments.clone(),
                    members: item.members.clone(),
                    alias: item.alias.clone(),
                    span: item.span,
                }),
                _ => items.push(self.lower_item(item, root_scope, &parent_path)),
            }
        }

        ResolveResult {
            module: hir::Module {
                package,
                imports,
                items,
            },
            symbols: self.symbols,
            diagnostics: self.diagnostics,
        }
    }

    fn lower_item(
        &mut self,
        item: &parser::Item,
        parent_scope: hir::ScopeId,
        parent_path: &[String],
    ) -> hir::Item {
        match item {
            parser::Item::Enum(item) => {
                hir::Item::Enum(self.lower_enum(item, parent_scope, parent_path))
            }
            parser::Item::Struct(item) => {
                hir::Item::Struct(self.lower_struct(item, parent_scope, parent_path))
            }
            parser::Item::TypeAlias(item) => {
                hir::Item::TypeAlias(self.lower_type_alias(item, parent_scope, parent_path))
            }
            parser::Item::Fn(item) => hir::Item::Fn(self.lower_fn(item, parent_scope, parent_path)),
            parser::Item::Impl(item) => {
                hir::Item::Impl(self.lower_impl(item, parent_scope, parent_path))
            }
            parser::Item::Import(_) => unreachable!("imports are separated before item lowering"),
            parser::Item::Error(span) => hir::Item::Error(*span),
        }
    }

    fn lower_enum(
        &mut self,
        item: &parser::EnumItem,
        parent_scope: hir::ScopeId,
        parent_path: &[String],
    ) -> hir::EnumItem {
        let item_path = append_path(parent_path, &item.name);
        let symbol = self.define_symbol(
            parent_scope,
            hir::Namespace::Type,
            &item.name,
            item_path.clone(),
            SymbolKind::Enum,
            item.public,
            item.span,
        );
        let scope = self.symbols.new_scope(parent_scope, item.name.clone());
        let binders = self.lower_generic_binders(&item.generics, scope, &item_path);
        let variants = item
            .variants
            .iter()
            .map(|variant| self.lower_enum_variant(variant, scope, &item_path))
            .collect();

        hir::EnumItem {
            symbol,
            scope,
            public: item.public,
            name: item.name.clone(),
            binders,
            variants,
            span: item.span,
        }
    }

    fn lower_enum_variant(
        &mut self,
        variant: &parser::EnumVariant,
        parent_scope: hir::ScopeId,
        parent_path: &[String],
    ) -> hir::EnumVariant {
        let variant_path = append_path(parent_path, &variant.name);
        let symbol = self.define_symbol(
            parent_scope,
            hir::Namespace::Value,
            &variant.name,
            variant_path.clone(),
            SymbolKind::EnumVariant,
            false,
            variant.span,
        );
        let scope = self.symbols.new_scope(parent_scope, variant.name.clone());
        let binders = self.lower_generic_binders(&variant.generics, scope, &variant_path);
        let kind = match &variant.kind {
            parser::VariantKind::Unit => hir::VariantKind::Unit,
            parser::VariantKind::Tuple(fields) => hir::VariantKind::Tuple(
                fields
                    .iter()
                    .map(|field| self.lower_field(field, scope, &variant_path))
                    .collect(),
            ),
            parser::VariantKind::Struct(fields) => hir::VariantKind::Struct(
                fields
                    .iter()
                    .map(|field| self.lower_field(field, scope, &variant_path))
                    .collect(),
            ),
        };

        hir::EnumVariant {
            symbol,
            scope,
            name: variant.name.clone(),
            binders,
            kind,
            where_clauses: variant.where_clauses.clone(),
            span: variant.span,
        }
    }

    fn lower_struct(
        &mut self,
        item: &parser::StructItem,
        parent_scope: hir::ScopeId,
        parent_path: &[String],
    ) -> hir::StructItem {
        let item_path = append_path(parent_path, &item.name);
        let symbol = self.define_symbol(
            parent_scope,
            hir::Namespace::Type,
            &item.name,
            item_path.clone(),
            SymbolKind::Struct,
            item.public,
            item.span,
        );
        let scope = self.symbols.new_scope(parent_scope, item.name.clone());
        let binders = self.lower_generic_binders(&item.generics, scope, &item_path);
        let fields = item
            .fields
            .iter()
            .map(|field| self.lower_field(field, scope, &item_path))
            .collect();

        hir::StructItem {
            symbol,
            scope,
            public: item.public,
            name: item.name.clone(),
            binders,
            fields,
            where_clauses: item.where_clauses.clone(),
            span: item.span,
        }
    }

    fn lower_type_alias(
        &mut self,
        item: &parser::TypeAliasItem,
        parent_scope: hir::ScopeId,
        parent_path: &[String],
    ) -> hir::TypeAliasItem {
        let item_path = append_path(parent_path, &item.name);
        let symbol = self.define_symbol(
            parent_scope,
            hir::Namespace::Type,
            &item.name,
            item_path.clone(),
            SymbolKind::TypeAlias,
            item.public,
            item.span,
        );
        let scope = self.symbols.new_scope(parent_scope, item.name.clone());
        let binders = self.lower_generic_binders(&item.generics, scope, &item_path);
        let ty = self.lower_type(&item.ty, scope, &item_path);

        hir::TypeAliasItem {
            symbol,
            scope,
            public: item.public,
            name: item.name.clone(),
            binders,
            ty,
            where_clauses: item.where_clauses.clone(),
            span: item.span,
        }
    }

    fn lower_fn(
        &mut self,
        item: &parser::FnItem,
        parent_scope: hir::ScopeId,
        parent_path: &[String],
    ) -> hir::FnItem {
        let item_path = append_path(parent_path, &item.name);
        let flavor = hir::FnFlavor::from(item.flavor);
        let symbol = self.define_symbol(
            parent_scope,
            hir::Namespace::Value,
            &item.name,
            item_path.clone(),
            SymbolKind::Function(flavor),
            item.public,
            item.span,
        );
        let scope = self.symbols.new_scope(parent_scope, item.name.clone());
        let mut binders = self.lower_generic_binders(&item.generics, scope, &item_path);
        binders.extend(
            item.params
                .iter()
                .map(|param| self.lower_param(param, scope, scope, &item_path)),
        );
        let return_type = item
            .return_type
            .as_ref()
            .map(|ty| self.lower_type(ty, scope, &item_path));

        hir::FnItem {
            symbol,
            scope,
            public: item.public,
            flavor,
            name: item.name.clone(),
            binders,
            return_type,
            requires: item.requires.clone(),
            ensures: item.ensures.clone(),
            body: item.body.clone(),
            span: item.span,
        }
    }

    fn lower_impl(
        &mut self,
        item: &parser::ImplItem,
        parent_scope: hir::ScopeId,
        parent_path: &[String],
    ) -> hir::ImplItem {
        let scope = self.symbols.new_scope(parent_scope, "impl");
        let binders = self.lower_generic_binders(&item.generics, scope, parent_path);
        let target = self.lower_type(&item.target, scope, parent_path);
        let items = item
            .items
            .iter()
            .map(|item| self.lower_item(item, scope, parent_path))
            .collect();

        hir::ImplItem {
            scope,
            binders,
            target,
            items,
            span: item.span,
        }
    }

    fn lower_generic_binders(
        &mut self,
        generics: &[parser::GenericParam],
        scope: hir::ScopeId,
        parent_path: &[String],
    ) -> Vec<hir::Binder> {
        generics
            .iter()
            .map(|generic| {
                let id = self.next_binder_id();
                match &generic.kind {
                    parser::GenericParamKind::Type => {
                        let symbol = self.define_binder_symbol(
                            scope,
                            hir::Namespace::Type,
                            &generic.name,
                            append_path(parent_path, &generic.name),
                            id,
                            hir::BinderMode::Implicit,
                            BinderSymbolKind::Type,
                            generic.span,
                        );
                        hir::Binder {
                            id,
                            symbol,
                            mode: hir::BinderMode::Implicit,
                            name: generic.name.clone(),
                            kind: hir::BinderKind::Type,
                            span: generic.span,
                        }
                    }
                    parser::GenericParamKind::Const { ty } => {
                        let ty = self.lower_type(ty, scope, parent_path);
                        let symbol = self.define_binder_symbol(
                            scope,
                            hir::Namespace::Value,
                            &generic.name,
                            append_path(parent_path, &generic.name),
                            id,
                            hir::BinderMode::Erased,
                            BinderSymbolKind::Value,
                            generic.span,
                        );
                        hir::Binder {
                            id,
                            symbol,
                            mode: hir::BinderMode::Erased,
                            name: generic.name.clone(),
                            kind: hir::BinderKind::Value { ty },
                            span: generic.span,
                        }
                    }
                }
            })
            .collect()
    }

    fn lower_param(
        &mut self,
        param: &parser::Param,
        type_scope: hir::ScopeId,
        binder_scope: hir::ScopeId,
        parent_path: &[String],
    ) -> hir::Binder {
        let id = self.next_binder_id();
        let mode = hir::BinderMode::from(param.mode);
        let ty = self.lower_type(&param.ty, type_scope, parent_path);
        let name = self.binder_name(&param.name, id);
        let symbol = self.define_binder_symbol(
            binder_scope,
            hir::Namespace::Value,
            &name,
            append_path(parent_path, &name),
            id,
            mode,
            BinderSymbolKind::Value,
            param.span,
        );

        hir::Binder {
            id,
            symbol,
            mode,
            name,
            kind: hir::BinderKind::Value { ty },
            span: param.span,
        }
    }

    fn lower_field(
        &mut self,
        field: &parser::Field,
        scope: hir::ScopeId,
        parent_path: &[String],
    ) -> hir::Field {
        hir::Field {
            public: field.public,
            name: field.name.clone(),
            ty: self.lower_type(&field.ty, scope, parent_path),
            span: field.span,
        }
    }

    fn lower_type(
        &mut self,
        ty: &parser::TypeExpr,
        scope: hir::ScopeId,
        parent_path: &[String],
    ) -> hir::Type {
        let kind = match &ty.kind {
            parser::TypeExprKind::Path(path) => hir::TypeKind::Path(hir::Path {
                segments: path.segments.clone(),
                span: path.span,
            }),
            parser::TypeExprKind::Generic { base, args } => hir::TypeKind::Apply {
                callee: Box::new(self.lower_type(base, scope, parent_path)),
                args: args
                    .iter()
                    .map(|arg| self.lower_type(arg, scope, parent_path))
                    .collect(),
                style: hir::TypeApplyStyle::Angle,
            },
            parser::TypeExprKind::Call { callee, args } => hir::TypeKind::Apply {
                callee: Box::new(self.lower_type(callee, scope, parent_path)),
                args: args
                    .iter()
                    .map(|arg| self.lower_type(arg, scope, parent_path))
                    .collect(),
                style: hir::TypeApplyStyle::Call,
            },
            parser::TypeExprKind::Tuple(items) => hir::TypeKind::Tuple(
                items
                    .iter()
                    .map(|item| self.lower_type(item, scope, parent_path))
                    .collect(),
            ),
            parser::TypeExprKind::Slice(element) => {
                hir::TypeKind::Slice(Box::new(self.lower_type(element, scope, parent_path)))
            }
            parser::TypeExprKind::Array { element, len } => hir::TypeKind::Array {
                element: Box::new(self.lower_type(element, scope, parent_path)),
                len: Box::new((**len).clone()),
            },
            parser::TypeExprKind::Pi { param, body } => {
                let body_scope = self.symbols.new_scope(scope, "pi");
                let param = self.lower_param(param, scope, body_scope, parent_path);
                let body = self.lower_type(body, body_scope, parent_path);
                hir::TypeKind::Pi {
                    param: Box::new(param),
                    body: Box::new(body),
                }
            }
            parser::TypeExprKind::Binary { op, left, right } => hir::TypeKind::Binary {
                op: *op,
                left: Box::new(self.lower_type(left, scope, parent_path)),
                right: Box::new(self.lower_type(right, scope, parent_path)),
            },
            parser::TypeExprKind::Hole(name) => hir::TypeKind::Hole(name.clone()),
            parser::TypeExprKind::Unknown => hir::TypeKind::Unknown,
        };

        hir::Type::new(kind, ty.span)
    }

    fn define_symbol(
        &mut self,
        scope: hir::ScopeId,
        namespace: hir::Namespace,
        name: &str,
        qualified_path: Vec<String>,
        kind: SymbolKind,
        public: bool,
        span: Span,
    ) -> hir::SymbolId {
        match self
            .symbols
            .define(scope, namespace, name, qualified_path, kind, public, span)
        {
            Ok(id) => id,
            Err(existing) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "AVR0001",
                        format!("duplicate {} symbol `{name}`", namespace.as_str()),
                    )
                    .with_span(span),
                );
                existing
            }
        }
    }

    fn define_binder_symbol(
        &mut self,
        scope: hir::ScopeId,
        namespace: hir::Namespace,
        name: &str,
        qualified_path: Vec<String>,
        id: hir::BinderId,
        mode: hir::BinderMode,
        kind: BinderSymbolKind,
        span: Span,
    ) -> hir::SymbolId {
        self.define_symbol(
            scope,
            namespace,
            name,
            qualified_path,
            SymbolKind::Binder { id, mode, kind },
            false,
            span,
        )
    }

    fn binder_name(&mut self, pattern: &parser::Pattern, id: hir::BinderId) -> String {
        match &pattern.kind {
            parser::PatternKind::Binding(name) => name.clone(),
            parser::PatternKind::Path(path) if path.segments.len() == 1 => path.segments[0].clone(),
            parser::PatternKind::Wildcard => format!("_{}", id.0),
            _ => {
                self.diagnostics.push(
                    Diagnostic::error("AVR0002", "binder must be a single name")
                        .with_span(pattern.span),
                );
                format!("_{}", id.0)
            }
        }
    }

    fn next_binder_id(&mut self) -> hir::BinderId {
        let id = hir::BinderId(self.next_binder);
        self.next_binder += 1;
        id
    }
}

fn append_path(parent: &[String], name: &str) -> Vec<String> {
    let mut path = parent.to_vec();
    path.push(name.to_string());
    path
}
