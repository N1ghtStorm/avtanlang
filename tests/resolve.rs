use avtanlang::hir::{self, BinderKind, BinderMode, Namespace, TypeKind};
use avtanlang::lexer;
use avtanlang::parser;
use avtanlang::resolve;
use avtanlang::source::{FileId, SourceFile};

#[test]
fn lowers_dependent_binders_into_hir() {
    let result = parse_and_resolve(include_str!("fixtures/parser/pass/dependent_binders.avtn"));

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);

    let hir::Item::TypeAlias(checked_index) = &result.module.items[0] else {
        panic!("expected CheckedIndex type alias");
    };
    assert_eq!(checked_index.binders.len(), 1);
    assert_eq!(checked_index.binders[0].mode, BinderMode::Erased);
    assert!(matches!(
        checked_index.binders[0].kind,
        BinderKind::Value { .. }
    ));

    let TypeKind::Pi { param, body } = &checked_index.ty.kind else {
        panic!("expected explicit pi binder");
    };
    assert_eq!(param.name, "i");
    assert_eq!(param.mode, BinderMode::Explicit);

    let TypeKind::Pi { param, body } = &body.kind else {
        panic!("expected auto pi binder");
    };
    assert_eq!(param.name, "p");
    assert_eq!(param.mode, BinderMode::Auto);

    let TypeKind::Pi { param, .. } = &body.kind else {
        panic!("expected erased pi binder");
    };
    assert_eq!(param.name, "same");
    assert_eq!(param.mode, BinderMode::Erased);

    let hir::Item::Fn(safe_index) = &result.module.items[1] else {
        panic!("expected safe_index function");
    };
    let modes = safe_index
        .binders
        .iter()
        .map(|binder| binder.mode)
        .collect::<Vec<_>>();
    assert_eq!(
        modes,
        [
            BinderMode::Implicit,
            BinderMode::Erased,
            BinderMode::Explicit,
            BinderMode::Explicit,
            BinderMode::Implicit,
            BinderMode::Implicit,
            BinderMode::Auto,
            BinderMode::Erased,
        ]
    );

    let root = result.symbols.root_scope();
    assert!(
        result
            .symbols
            .find(root, Namespace::Type, "CheckedIndex")
            .is_some()
    );
    assert!(
        result
            .symbols
            .find(root, Namespace::Value, "safe_index")
            .is_some()
    );
    assert!(
        result
            .symbols
            .find(safe_index.scope, Namespace::Type, "T")
            .is_some()
    );
    assert!(
        result
            .symbols
            .find(safe_index.scope, Namespace::Value, "N")
            .is_some()
    );
    assert!(
        result
            .symbols
            .find(safe_index.scope, Namespace::Value, "p")
            .is_some()
    );
}

#[test]
fn separates_type_and_value_namespaces() {
    let result = parse_and_resolve(
        r#"
package examples.namespaces

type Same = Nat;

fn Same() {}
"#,
    );

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);

    let root = result.symbols.root_scope();
    assert!(result.symbols.find(root, Namespace::Type, "Same").is_some());
    assert!(
        result
            .symbols
            .find(root, Namespace::Value, "Same")
            .is_some()
    );
}

#[test]
fn reports_duplicate_symbols_in_the_same_namespace() {
    let result = parse_and_resolve(
        r#"
fn same() {}
fn same() {}
"#,
    );

    assert_eq!(
        result
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>(),
        ["AVR0001"]
    );
}

fn parse_and_resolve(text: &str) -> resolve::ResolveResult {
    let file = SourceFile::new(FileId(0), "test.avtn", text);
    let lexed = lexer::lex(&file);
    assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
    let parsed = parser::parse_tokens(&lexed.tokens);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    resolve::resolve_module(&parsed.module)
}
