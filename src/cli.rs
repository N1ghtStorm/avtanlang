use std::env;
use std::fs;
use std::path::PathBuf;

use crate::diagnostics::render_diagnostic;
use crate::lexer;
use crate::parser;
use crate::resolve;
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
        "parse" => {
            let mut format = ParseFormat::Pretty;
            let mut path = None;

            for arg in args {
                match arg.as_str() {
                    "--pretty" => format = ParseFormat::Pretty,
                    "--debug" => format = ParseFormat::Debug,
                    value if value.starts_with('-') => {
                        eprintln!("error: unknown parse option `{value}`");
                        print_usage();
                        return 2;
                    }
                    _ if path.is_none() => path = Some(arg),
                    _ => {
                        eprintln!("error: `avtan parse` accepts exactly one input file");
                        print_usage();
                        return 2;
                    }
                }
            }

            let Some(path) = path else {
                eprintln!("error: missing input file for `avtan parse`");
                print_usage();
                return 2;
            };

            parse_file(PathBuf::from(path), format)
        }
        "resolve" => {
            let Some(path) = args.next() else {
                eprintln!("error: missing input file for `avtan resolve`");
                print_usage();
                return 2;
            };

            if args.next().is_some() {
                eprintln!("error: `avtan resolve` accepts exactly one input file");
                print_usage();
                return 2;
            }

            resolve_file(PathBuf::from(path))
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ParseFormat {
    Pretty,
    Debug,
}

fn parse_file(path: PathBuf, format: ParseFormat) -> i32 {
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("error: failed to read {}: {error}", path.display());
            return 1;
        }
    };

    let file = SourceFile::new(FileId(0), path, text);
    let lexed = lexer::lex(&file);
    let parsed = parser::parse_tokens(&lexed.tokens);

    for diagnostic in &lexed.diagnostics {
        eprint!("{}", render_diagnostic(diagnostic, Some(&file)));
    }
    for diagnostic in &parsed.diagnostics {
        eprint!("{}", render_diagnostic(diagnostic, Some(&file)));
    }

    if lexed.diagnostics.is_empty() && parsed.diagnostics.is_empty() {
        match format {
            ParseFormat::Pretty => print!("{}", parser::dump_module(&parsed.module)),
            ParseFormat::Debug => println!("{:#?}", parsed.module),
        }
        0
    } else {
        1
    }
}

fn resolve_file(path: PathBuf) -> i32 {
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("error: failed to read {}: {error}", path.display());
            return 1;
        }
    };

    let file = SourceFile::new(FileId(0), path, text);
    let lexed = lexer::lex(&file);
    let parsed = parser::parse_tokens(&lexed.tokens);

    for diagnostic in &lexed.diagnostics {
        eprint!("{}", render_diagnostic(diagnostic, Some(&file)));
    }
    for diagnostic in &parsed.diagnostics {
        eprint!("{}", render_diagnostic(diagnostic, Some(&file)));
    }

    if !lexed.diagnostics.is_empty() || !parsed.diagnostics.is_empty() {
        return 1;
    }

    let resolved = resolve::resolve_module(&parsed.module);
    for diagnostic in &resolved.diagnostics {
        eprint!("{}", render_diagnostic(diagnostic, Some(&file)));
    }

    if resolved.diagnostics.is_empty() {
        print!("{}", resolve::dump_symbols(&resolved.symbols));
        0
    } else {
        1
    }
}

fn print_usage() {
    eprintln!("usage:");
    eprintln!("  avtan lex <file.avtn>");
    eprintln!("  avtan parse [--pretty|--debug] <file.avtn>");
    eprintln!("  avtan resolve <file.avtn>");
}
