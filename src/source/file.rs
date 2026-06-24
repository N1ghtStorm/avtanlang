use std::path::{Path, PathBuf};

use super::{FileId, Span};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceLocation {
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug)]
pub struct SourceFile {
    id: FileId,
    path: PathBuf,
    text: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    pub fn new(id: FileId, path: impl Into<PathBuf>, text: impl Into<String>) -> Self {
        let text = text.into();
        let line_starts = compute_line_starts(&text);

        Self {
            id,
            path: path.into(),
            text,
            line_starts,
        }
    }

    pub fn id(&self) -> FileId {
        self.id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn span(&self, start: usize, end: usize) -> Span {
        Span::new(self.id, start, end)
    }

    pub fn line_col(&self, byte_index: usize) -> SourceLocation {
        let mut byte_index = byte_index.min(self.text.len());
        while !self.text.is_char_boundary(byte_index) {
            byte_index -= 1;
        }

        let line_index = match self.line_starts.binary_search(&byte_index) {
            Ok(index) => index,
            Err(next_index) => next_index.saturating_sub(1),
        };
        let line_start = self.line_starts[line_index];
        let column = self.text[line_start..byte_index].chars().count() + 1;

        SourceLocation {
            line: line_index + 1,
            column,
        }
    }

    pub fn line_text(&self, line: usize) -> Option<&str> {
        if line == 0 || line > self.line_starts.len() {
            return None;
        }

        let start = self.line_starts[line - 1];
        let end = self
            .line_starts
            .get(line)
            .copied()
            .unwrap_or(self.text.len());
        Some(self.text[start..end].trim_end_matches(['\r', '\n']))
    }
}

#[derive(Default, Debug)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, path: impl Into<PathBuf>, text: impl Into<String>) -> FileId {
        let id = FileId(self.files.len() as u32);
        self.files.push(SourceFile::new(id, path, text));
        id
    }

    pub fn get(&self, id: FileId) -> Option<&SourceFile> {
        self.files.get(id.0 as usize)
    }

    pub fn files(&self) -> &[SourceFile] {
        &self.files
    }
}

fn compute_line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];

    for (index, byte) in text.bytes().enumerate() {
        if byte == b'\n' && index + 1 < text.len() {
            starts.push(index + 1);
        }
    }

    starts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_byte_offsets_to_lines_and_columns() {
        let file = SourceFile::new(FileId(0), "sample.avtn", "one\nтри\nlast");

        assert_eq!(file.line_col(0), SourceLocation { line: 1, column: 1 });
        assert_eq!(file.line_col(5), SourceLocation { line: 2, column: 1 });
        assert_eq!(file.line_col(9), SourceLocation { line: 2, column: 3 });
    }

    #[test]
    fn extracts_line_text_without_newline() {
        let file = SourceFile::new(FileId(0), "sample.avtn", "one\r\ntwo\nthree");

        assert_eq!(file.line_text(1), Some("one"));
        assert_eq!(file.line_text(2), Some("two"));
        assert_eq!(file.line_text(3), Some("three"));
        assert_eq!(file.line_text(4), None);
    }
}
