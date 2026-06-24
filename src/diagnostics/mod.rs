mod error;
mod report;

pub use error::{Diagnostic, Label, Severity};
pub use report::render_diagnostic;
