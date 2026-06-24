use std::env;
use std::fs;
use std::path::PathBuf;

use crate::diagnostics::render_diagnostic;
use crate::lexer;
use crate::source::{FileId, SourceFile};

pub fn run() -> i32 {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return 2;
    };

    match command.as_str() {
        "lex" => {
            let Some(path) = args.next() else {
                eprintln!("error: missing input file for `avtan lex`");
                print_usage();
                return 2;
            };

            if args.next().is_some() {
                eprintln!("error: `avtan lex` accepts exactly one input file");
                print_usage();
                return 2;
            }

            lex_file(PathBuf::from(path))
        }
        "help" | "-h" | "--help" => {
            print_usage();
            0
        }
        other => {
            eprintln!("error: unknown command `{other}`");
            print_usage();
            2
        }
    }
}

fn lex_file(path: PathBuf) -> i32 {
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("error: failed to read {}: {error}", path.display());
            return 1;
        }
    };

    let file = SourceFile::new(FileId(0), path, text);
    let result = lexer::lex(&file);

    for token in &result.tokens {
        let location = file.line_col(token.span.start);
        println!(
            "{}:{}\t{}\t{}..{}",
            location.line, location.column, token.kind, token.span.start, token.span.end
        );
    }

    for diagnostic in &result.diagnostics {
        eprint!("{}", render_diagnostic(diagnostic, Some(&file)));
    }

    if result.diagnostics.is_empty() { 0 } else { 1 }
}

fn print_usage() {
    eprintln!("usage:");
    eprintln!("  avtan lex <file.avtn>");
}
