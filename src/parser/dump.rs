use std::fmt::Write;

use super::ast::*;

pub fn dump_module(module: &Module) -> String {
    let mut dumper = Dumper {
        output: String::new(),
        indent: 0,
    };
    dumper.module(module);
    dumper.output
}

struct Dumper {
    output: String,
    indent: usize,
}

impl Dumper {
    fn module(&mut self, module: &Module) {
        if let Some(package) = &module.package {
            self.line(format_args!("module {}", package_to_string(package)));
        } else {
            self.line(format_args!("module <anonymous>"));
        }

        self.indented(|this| {
            for item in &module.items {
                this.item(item);
            }
        });
    }

    fn item(&mut self, item: &Item) {
        match item {
            Item::Import(item) => self.import(item),
            Item::Enum(item) => self.enum_item(item),
            Item::Struct(item) => self.struct_item(item),
            Item::TypeAlias(item) => self.type_alias(item),
            Item::Fn(item) => self.fn_item(item),
            Item::Impl(item) => self.impl_item(item),
            Item::Error(_) => self.line(format_args!("error-item")),
        }
    }

    fn import(&mut self, item: &ImportItem) {
        self.attributes(&item.attributes);
        let mut line = format!("import {}", package_to_string(&item.path));
        if !item.members.is_empty() {
            line.push_str(".{");
            line.push_str(&item.members.join(", "));
            line.push('}');
        }
        if let Some(alias) = &item.alias {
            line.push_str(" as ");
            line.push_str(alias);
        }
        self.line(format_args!("{line}"));
    }

    fn enum_item(&mut self, item: &EnumItem) {
        self.attributes(&item.attributes);
        self.line(format_args!(
            "{}enum {}{}",
            visibility(item.public),
            item.name,
            generics_to_string(&item.generics)
        ));
        self.indented(|this| {
            for variant in &item.variants {
                this.variant(variant);
            }
        });
    }

    fn variant(&mut self, variant: &EnumVariant) {
        self.attributes(&variant.attributes);
        let mut line = format!(
            "variant {}{} {}",
            variant.name,
            generics_to_string(&variant.generics),
            variant_kind_to_string(&variant.kind)
        );
        append_where(&mut line, &variant.where_clauses);
        self.line(format_args!("{line}"));
    }

    fn struct_item(&mut self, item: &StructItem) {
        self.attributes(&item.attributes);
        let mut line = format!(
            "{}struct {}{}",
            visibility(item.public),
            item.name,
            generics_to_string(&item.generics)
        );
        append_where(&mut line, &item.where_clauses);
        self.line(format_args!("{line}"));
        self.indented(|this| {
            for field in &item.fields {
                this.field(field);
            }
        });
    }

    fn type_alias(&mut self, item: &TypeAliasItem) {
        self.attributes(&item.attributes);
        let mut line = format!(
            "{}type {}{} = {}",
            visibility(item.public),
            item.name,
            generics_to_string(&item.generics),
            type_to_string(&item.ty)
        );
        append_where(&mut line, &item.where_clauses);
        self.line(format_args!("{line}"));
    }

    fn fn_item(&mut self, item: &FnItem) {
        self.attributes(&item.attributes);
        let mut line = format!(
            "{}{}fn {}{}({})",
            visibility(item.public),
            fn_flavor(item.flavor),
            item.name,
            generics_to_string(&item.generics),
            params_to_string(&item.params)
        );
        if let Some(return_type) = &item.return_type {
            line.push_str(" -> ");
            line.push_str(&type_to_string(return_type));
        }
        append_contracts(&mut line, "requires", &item.requires);
        append_contracts(&mut line, "ensures", &item.ensures);
        self.line(format_args!("{line}"));

        if let Some(body) = &item.body {
            self.block(body);
        }
    }

    fn impl_item(&mut self, item: &ImplItem) {
        self.attributes(&item.attributes);
        self.line(format_args!(
            "impl{} {}",
            generics_to_string(&item.generics),
            type_to_string(&item.target)
        ));
        self.indented(|this| {
            for item in &item.items {
                this.item(item);
            }
        });
    }

    fn field(&mut self, field: &Field) {
        self.attributes(&field.attributes);
        let name = field.name.as_deref().unwrap_or("_");
        self.line(format_args!(
            "{}field {}: {}",
            visibility(field.public),
            name,
            type_to_string(&field.ty)
        ));
    }

    fn block(&mut self, block: &Block) {
        self.indented(|this| {
            this.line(format_args!("block"));
            this.indented(|this| {
                for stmt in &block.statements {
                    this.stmt(stmt);
                }
            });
        });
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let {
                pattern, ty, value, ..
            } => {
                let mut line = format!("let {}", pattern_to_string(pattern));
                if let Some(ty) = ty {
                    line.push_str(": ");
                    line.push_str(&type_to_string(ty));
                }
                if let Some(value) = value {
                    line.push_str(" = ");
                    line.push_str(&expr_to_string(value));
                }
                self.line(format_args!("{line}"));
            }
            Stmt::Expr(expr) => self.expr(expr),
        }
    }

    fn expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Block(block) => {
                self.line(format_args!("expr block"));
                self.block(block);
            }
            ExprKind::Match { scrutinee, arms } => {
                self.line(format_args!("match {}", expr_to_string(scrutinee)));
                self.indented(|this| {
                    for arm in arms {
                        this.line(format_args!(
                            "arm {} => {}",
                            pattern_to_string(&arm.pattern),
                            expr_to_string(&arm.body)
                        ));
                    }
                });
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.line(format_args!("if {}", expr_to_string(condition)));
                self.indented(|this| {
                    this.line(format_args!("then"));
                    this.block(then_branch);
                    if let Some(else_branch) = else_branch {
                        this.line(format_args!("else"));
                        match &else_branch.kind {
                            ExprKind::Block(block) => this.block(block),
                            _ => this.indented(|this| this.expr(else_branch)),
                        }
                    }
                });
            }
            ExprKind::While { condition, body } => {
                self.line(format_args!("while {}", expr_to_string(condition)));
                self.block(body);
            }
            ExprKind::For {
                pattern,
                iterable,
                body,
            } => {
                self.line(format_args!(
                    "for {} in {}",
                    pattern_to_string(pattern),
                    expr_to_string(iterable)
                ));
                self.block(body);
            }
            ExprKind::Loop { body } => {
                self.line(format_args!("loop"));
                self.block(body);
            }
            _ => self.line(format_args!("expr {}", expr_to_string(expr))),
        }
    }

    fn attributes(&mut self, attributes: &[Attribute]) {
        for attribute in attributes {
            self.line(format_args!("attr {}", attribute_to_string(attribute)));
        }
    }

    fn indented(&mut self, f: impl FnOnce(&mut Self)) {
        self.indent += 1;
        f(self);
        self.indent -= 1;
    }

    fn line(&mut self, args: std::fmt::Arguments<'_>) {
        for _ in 0..self.indent {
            self.output.push_str("  ");
        }
        let _ = self.output.write_fmt(args);
        self.output.push('\n');
    }
}

fn visibility(public: bool) -> &'static str {
    if public { "pub " } else { "" }
}

fn fn_flavor(flavor: FnFlavor) -> &'static str {
    match flavor {
        FnFlavor::Plain => "",
        FnFlavor::Proof => "proof ",
        FnFlavor::Total => "total ",
        FnFlavor::Partial => "partial ",
    }
}

fn generics_to_string(generics: &[GenericParam]) -> String {
    if generics.is_empty() {
        return String::new();
    }

    format!(
        "<{}>",
        generics
            .iter()
            .map(generic_to_string)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn generic_to_string(generic: &GenericParam) -> String {
    match &generic.kind {
        GenericParamKind::Type => generic.name.clone(),
        GenericParamKind::Const { ty } => {
            format!("const {}: {}", generic.name, type_to_string(ty))
        }
    }
}

fn params_to_string(params: &[Param]) -> String {
    params
        .iter()
        .map(param_to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn param_to_string(param: &Param) -> String {
    let text = format!(
        "{}: {}",
        pattern_to_string(&param.name),
        type_to_string(&param.ty)
    );

    match param.mode {
        BinderMode::Explicit => text,
        BinderMode::Implicit => format!("{{implicit {text}}}"),
        BinderMode::Auto => format!("{{auto {text}}}"),
        BinderMode::Erased => format!("{{erased {text}}}"),
    }
}

fn variant_kind_to_string(kind: &VariantKind) -> String {
    match kind {
        VariantKind::Unit => "unit".to_string(),
        VariantKind::Tuple(fields) => format!(
            "tuple({})",
            fields
                .iter()
                .map(|field| type_to_string(&field.ty))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        VariantKind::Struct(fields) => format!(
            "struct({})",
            fields
                .iter()
                .map(field_to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn field_to_string(field: &Field) -> String {
    match &field.name {
        Some(name) => format!("{name}: {}", type_to_string(&field.ty)),
        None => type_to_string(&field.ty),
    }
}

fn append_where(line: &mut String, clauses: &[Expr]) {
    if !clauses.is_empty() {
        line.push_str(" where ");
        line.push_str(
            &clauses
                .iter()
                .map(expr_to_string)
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
}

fn append_contracts(line: &mut String, keyword: &str, clauses: &[Expr]) {
    for clause in clauses {
        line.push(' ');
        line.push_str(keyword);
        line.push(' ');
        line.push_str(&expr_to_string(clause));
    }
}

fn attribute_to_string(attribute: &Attribute) -> String {
    let mut text = package_to_string(&attribute.path);
    if !attribute.args.is_empty() {
        text.push('(');
        text.push_str(
            &attribute
                .args
                .iter()
                .map(attribute_arg_to_string)
                .collect::<Vec<_>>()
                .join(", "),
        );
        text.push(')');
    }
    text
}

fn attribute_arg_to_string(arg: &AttributeArg) -> String {
    match arg {
        AttributeArg::Expr(expr) => expr_to_string(expr),
        AttributeArg::NameValue { name, value, .. } => {
            format!("{name} = {}", expr_to_string(value))
        }
    }
}

fn type_to_string(ty: &TypeExpr) -> String {
    match &ty.kind {
        TypeExprKind::Path(path) => path_to_string(path),
        TypeExprKind::Generic { base, args } => format!(
            "{}<{}>",
            type_to_string(base),
            args.iter()
                .map(type_to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeExprKind::Call { callee, args } => format!(
            "{}({})",
            type_to_string(callee),
            args.iter()
                .map(type_to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeExprKind::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .map(type_to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeExprKind::Slice(element) => format!("[]{}", type_to_string(element)),
        TypeExprKind::Array { element, len } => {
            format!("[{}; {}]", type_to_string(element), expr_to_string(len))
        }
        TypeExprKind::Pi { param, body } => {
            format!("{} -> {}", pi_param_to_string(param), type_to_string(body))
        }
        TypeExprKind::Binary { op, left, right } => {
            format!(
                "{} {} {}",
                type_to_string(left),
                binary_op_to_string(*op),
                type_to_string(right)
            )
        }
        TypeExprKind::Hole(name) => format!("?{name}"),
        TypeExprKind::Unknown => "<unknown>".to_string(),
    }
}

fn pi_param_to_string(param: &Param) -> String {
    match param.mode {
        BinderMode::Explicit => format!("({})", param_to_string(param)),
        BinderMode::Implicit | BinderMode::Auto | BinderMode::Erased => param_to_string(param),
    }
}

fn expr_to_string(expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::Path(path) => path_to_string(path),
        ExprKind::Int(value) => value.clone(),
        ExprKind::Float(value) => value.clone(),
        ExprKind::String(value) => format!("{value:?}"),
        ExprKind::Char(value) => format!("'{value}'"),
        ExprKind::Bool(value) => value.to_string(),
        ExprKind::Hole(name) => format!("?{name}"),
        ExprKind::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .map(expr_to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ExprKind::Call { callee, args } => format!(
            "{}({})",
            expr_to_string(callee),
            args.iter()
                .map(expr_to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ExprKind::Field { base, name } => format!("{}.{}", expr_to_string(base), name),
        ExprKind::Index { base, index } => {
            format!("{}[{}]", expr_to_string(base), expr_to_string(index))
        }
        ExprKind::Binary { op, left, right } => {
            format!(
                "{} {} {}",
                expr_to_string(left),
                binary_op_to_string(*op),
                expr_to_string(right)
            )
        }
        ExprKind::Block(_) => "{...}".to_string(),
        ExprKind::Match { .. } => "match ...".to_string(),
        ExprKind::If { .. } => "if ...".to_string(),
        ExprKind::While { .. } => "while ...".to_string(),
        ExprKind::For { .. } => "for ...".to_string(),
        ExprKind::Loop { .. } => "loop ...".to_string(),
        ExprKind::Rewrite { proof, body } => {
            format!(
                "rewrite {} in {}",
                expr_to_string(proof),
                expr_to_string(body)
            )
        }
        ExprKind::Return(Some(value)) => format!("return {}", expr_to_string(value)),
        ExprKind::Return(None) => "return".to_string(),
        ExprKind::Break(Some(value)) => format!("break {}", expr_to_string(value)),
        ExprKind::Break(None) => "break".to_string(),
        ExprKind::Continue => "continue".to_string(),
        ExprKind::Impossible => "impossible".to_string(),
        ExprKind::Unknown => "<unknown>".to_string(),
    }
}

fn pattern_to_string(pattern: &Pattern) -> String {
    match &pattern.kind {
        PatternKind::Wildcard => "_".to_string(),
        PatternKind::Binding(name) => name.clone(),
        PatternKind::Path(path) => path_to_string(path),
        PatternKind::Tuple { path, elements } => {
            let prefix = path.as_ref().map(path_to_string).unwrap_or_default();
            format!(
                "{}({})",
                prefix,
                elements
                    .iter()
                    .map(pattern_to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        PatternKind::Struct { path, fields, rest } => {
            let mut parts = fields
                .iter()
                .map(pattern_field_to_string)
                .collect::<Vec<_>>();
            if *rest {
                parts.push("..".to_string());
            }
            format!("{} {{ {} }}", path_to_string(path), parts.join(", "))
        }
        PatternKind::Int(value) => value.clone(),
        PatternKind::Unknown => "<unknown>".to_string(),
    }
}

fn pattern_field_to_string(field: &PatternField) -> String {
    match &field.pattern {
        Some(pattern) => format!("{}: {}", field.name, pattern_to_string(pattern)),
        None => field.name.clone(),
    }
}

fn path_to_string(path: &Path) -> String {
    path.segments.join("::")
}

fn package_to_string(path: &Path) -> String {
    path.segments.join(".")
}

fn binary_op_to_string(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::LtEq => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::GtEq => ">=",
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
    }
}
