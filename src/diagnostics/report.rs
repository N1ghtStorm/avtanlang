use crate::source::{SourceFile, Span};

use super::Diagnostic;

pub fn render_diagnostic(diagnostic: &Diagnostic, file: Option<&SourceFile>) -> String {
    let mut output = String::new();
    output.push_str(diagnostic.severity.as_str());
    output.push('[');
    output.push_str(diagnostic.code);
    output.push_str("]: ");
    output.push_str(&diagnostic.message);
    output.push('\n');

    if let (Some(span), Some(file)) = (diagnostic.span, file) {
        render_span(&mut output, file, span, "");
    }

    for label in &diagnostic.labels {
        if let Some(file) = file {
            render_span(&mut output, file, label.span, &label.message);
        }
    }

    for note in &diagnostic.notes {
        output.push_str("note: ");
        output.push_str(note);
        output.push('\n');
    }

    output
}

fn render_span(output: &mut String, file: &SourceFile, span: Span, label: &str) {
    let location = file.line_col(span.start);
    output.push_str(" --> ");
    output.push_str(&file.path().display().to_string());
    output.push(':');
    output.push_str(&location.line.to_string());
    output.push(':');
    output.push_str(&location.column.to_string());
    output.push('\n');

    if let Some(line_text) = file.line_text(location.line) {
        output.push_str("  |\n");
        output.push_str(&location.line.to_string());
        output.push_str(" | ");
        output.push_str(line_text);
        output.push('\n');
        output.push_str("  | ");

        let start_column = location.column.saturating_sub(1);
        output.push_str(&" ".repeat(start_column));
        let marker_len = span.len().max(1);
        output.push_str(&"^".repeat(marker_len.min(80)));
        if !label.is_empty() {
            output.push(' ');
            output.push_str(label);
        }
        output.push('\n');
    }
}
