use std::fs;
use std::path::{Path, PathBuf};

use avtanlang::lexer;
use avtanlang::parser;
use avtanlang::source::{FileId, SourceFile};

#[test]
fn pass_fixtures_match_stable_dumps() {
    for path in fixture_paths("tests/fixtures/parser/pass", "avtn") {
        let expected_path = path.with_extension("dump");
        let parsed = parse_fixture(&path);
        let codes = diagnostic_codes(&parsed.lex_diagnostics, &parsed.parse_diagnostics);

        assert!(
            codes.is_empty(),
            "{} produced diagnostics: {codes:?}",
            path.display()
        );

        let actual = parser::dump_module(&parsed.module);
        let expected = fs::read_to_string(&expected_path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", expected_path.display()));

        assert_eq!(actual, expected, "dump mismatch for {}", path.display());
    }
}

#[test]
fn fail_fixtures_match_expected_diagnostic_codes() {
    for path in fixture_paths("tests/fixtures/parser/fail", "avtn") {
        let expected_path = path.with_extension("errors");
        let parsed = parse_fixture(&path);
        let actual = diagnostic_codes(&parsed.lex_diagnostics, &parsed.parse_diagnostics);
        let expected = fs::read_to_string(&expected_path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", expected_path.display()))
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(str::to_string)
            .collect::<Vec<_>>();

        assert_eq!(
            actual,
            expected,
            "diagnostic code mismatch for {}",
            path.display()
        );
    }
}

struct ParsedFixture {
    module: parser::Module,
    lex_diagnostics: Vec<avtanlang::diagnostics::Diagnostic>,
    parse_diagnostics: Vec<avtanlang::diagnostics::Diagnostic>,
}

fn parse_fixture(path: &Path) -> ParsedFixture {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    let file = SourceFile::new(FileId(0), path, text);
    let lexed = lexer::lex(&file);
    let parsed = parser::parse_tokens(&lexed.tokens);

    ParsedFixture {
        module: parsed.module,
        lex_diagnostics: lexed.diagnostics,
        parse_diagnostics: parsed.diagnostics,
    }
}

fn fixture_paths(dir: &str, extension: &str) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(dir);
    let mut paths = fs::read_dir(&root)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", root.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|error| panic!("failed to read fixture entry: {error}"))
                .path()
        })
        .filter(|path| path.extension().is_some_and(|found| found == extension))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn diagnostic_codes(
    lex_diagnostics: &[avtanlang::diagnostics::Diagnostic],
    parse_diagnostics: &[avtanlang::diagnostics::Diagnostic],
) -> Vec<String> {
    lex_diagnostics
        .iter()
        .chain(parse_diagnostics)
        .map(|diagnostic| diagnostic.code.to_string())
        .collect()
}
