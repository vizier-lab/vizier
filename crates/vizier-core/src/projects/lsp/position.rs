//! Positions and locations: the agent speaks 1-based lines and Unicode-scalar columns, a server
//! speaks 0-based lines and `utf-8` or `utf-16` offsets (R15), and a result never carries an
//! absolute host path (FR-022).

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Value, json};
use url::Url;

/// The offset unit negotiated with a server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Encoding {
    Utf8,
    #[default]
    Utf16,
}

impl Encoding {
    /// Read `capabilities.positionEncoding`; absent means `utf-16`.
    pub fn from_capability(value: Option<&str>) -> Self {
        match value {
            Some("utf-8") => Encoding::Utf8,
            _ => Encoding::Utf16,
        }
    }

    fn width(self, c: char) -> usize {
        match self {
            Encoding::Utf8 => c.len_utf8(),
            Encoding::Utf16 => c.len_utf16(),
        }
    }
}

/// The text of a line without its line ending.
pub fn strip_eol(line: &str) -> &str {
    line.trim_end_matches(['\n', '\r'])
}

/// 1-based scalar column to a server offset on the same line. A column past the end clamps to it.
pub fn column_to_lsp(line_text: &str, column: usize, enc: Encoding) -> u32 {
    let line = strip_eol(line_text);
    line.chars()
        .take(column.saturating_sub(1))
        .map(|c| enc.width(c))
        .sum::<usize>() as u32
}

/// A server offset to a 1-based scalar column. An offset inside a character rounds up to it.
pub fn column_from_lsp(line_text: &str, offset: u32, enc: Encoding) -> usize {
    let line = strip_eol(line_text);
    let mut seen = 0usize;
    let mut column = 1usize;
    for c in line.chars() {
        if seen >= offset as usize {
            return column;
        }
        seen += enc.width(c);
        column += 1;
    }
    column
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Find `symbol` on a line, on word boundaries, and return its 1-based scalar column.
/// `occurrence` is 1-based; `None` means the first.
pub fn locate_symbol(line_text: &str, symbol: &str, occurrence: Option<usize>) -> Option<usize> {
    if symbol.is_empty() {
        return None;
    }
    let line = strip_eol(line_text);
    let wanted = occurrence.unwrap_or(1).max(1);
    let mut found = 0usize;
    let mut from = 0usize;
    while let Some(rel) = line[from..].find(symbol) {
        let start = from + rel;
        let end = start + symbol.len();
        let before_ok = line[..start].chars().next_back().is_none_or(|c| !is_word(c))
            || !symbol.chars().next().is_some_and(is_word);
        let after_ok = line[end..].chars().next().is_none_or(|c| !is_word(c))
            || !symbol.chars().next_back().is_some_and(is_word);
        if before_ok && after_ok {
            found += 1;
            if found == wanted {
                return Some(line[..start].chars().count() + 1);
            }
        }
        from = start + line[start..].chars().next().map_or(1, char::len_utf8);
    }
    None
}

/// `file://` URI for an absolute path.
pub fn path_to_uri(path: &Path) -> Option<String> {
    Url::from_file_path(path).ok().map(|u| u.to_string())
}

/// Absolute path for a `file://` URI.
///
/// A URI with no drive letter (`file:///elsewhere/x.rs`) has no absolute path on Windows, and
/// `to_file_path` refuses it, which would drop a location a server reported. It is kept as the
/// rootless path `/elsewhere/x.rs` instead, so it lands outside the project and is named as such
/// rather than disappearing.
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let url = Url::parse(uri).ok()?;
    if let Ok(path) = url.to_file_path() {
        // The same form a canonicalised folder has: a `\?\` root, so it compares equal to one.
        return Some(match std::fs::canonicalize(&path) {
            Ok(canonical) => canonical,
            Err(_) => path,
        });
    }
    let text = url.path();
    (!text.is_empty()).then(|| PathBuf::from(text))
}

/// Where a server's location lands, as the agent sees it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Location {
    Inside {
        path: String,
        line: usize,
        character: usize,
        #[serde(skip_serializing_if = "String::is_empty")]
        preview: String,
    },
    Outside {
        outside_project: bool,
        file: String,
        line: usize,
    },
}

/// Turn a path inside `cwd` into a `/`-separated relative one.
///
/// Compared the way Windows compares: a `file://` URI yields `C:/…/src/a.rs` and a canonicalised
/// folder yields `\\?\C:\…`, which are the same place and are not equal as paths.
pub fn relative(cwd: &Path, path: &Path) -> Option<String> {
    let cwd = strip_verbatim(cwd);
    let path = strip_verbatim(path);
    let rel = path.strip_prefix(&cwd).ok()?;
    let text = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/");
    Some(if text.is_empty() { ".".into() } else { text })
}

/// `path` without Windows' verbatim prefix, so `\\?\C:\x` and `C:/x` compare as one folder.
fn strip_verbatim(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) => PathBuf::from(rest),
        None => path.to_path_buf(),
    }
}

/// Shorten a source line for a result.
pub fn preview(line_text: &str) -> String {
    let line = strip_eol(line_text).trim();
    if line.chars().count() <= 160 {
        return line.to_owned();
    }
    let cut: String = line.chars().take(157).collect();
    format!("{cut}...")
}

/// Map an LSP `Location`-like `{uri, range}` to the agent's form. `read_line(path, line0)` supplies
/// the text for the preview and the column. `None` drops the location (under `.vizier/`, or a URI
/// that is not a file).
pub fn map_location(
    cwd: &Path,
    uri: &str,
    line0: u32,
    offset: u32,
    enc: Encoding,
    read_line: &dyn Fn(&Path, u32) -> Option<String>,
) -> Option<Location> {
    let path = uri_to_path(uri)?;
    match relative(cwd, &path) {
        Some(rel) => {
            if rel == ".vizier" || rel.starts_with(".vizier/") {
                return None;
            }
            let text = read_line(&path, line0).unwrap_or_default();
            Some(Location::Inside {
                character: column_from_lsp(&text, offset, enc),
                preview: preview(&text),
                path: rel,
                line: line0 as usize + 1,
            })
        }
        None => Some(Location::Outside {
            outside_project: true,
            file: path.file_name()?.to_string_lossy().into_owned(),
            line: line0 as usize + 1,
        }),
    }
}

/// `{ "line", "character" }` of a position, for a request.
pub fn lsp_position(line0: u32, character: u32) -> Value {
    json!({ "line": line0, "character": character })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_counts_an_emoji_as_two_units() {
        let line = "a😀b";
        // 'b' is the third scalar: column 3
        assert_eq!(column_to_lsp(line, 3, Encoding::Utf16), 3);
        assert_eq!(column_to_lsp(line, 3, Encoding::Utf8), 5);
        assert_eq!(column_from_lsp(line, 3, Encoding::Utf16), 3);
        assert_eq!(column_from_lsp(line, 5, Encoding::Utf8), 3);
    }

    #[test]
    fn cjk_is_one_utf16_unit_and_three_utf8_bytes() {
        let line = "日本語x";
        assert_eq!(column_to_lsp(line, 4, Encoding::Utf16), 3);
        assert_eq!(column_to_lsp(line, 4, Encoding::Utf8), 9);
        assert_eq!(column_from_lsp(line, 9, Encoding::Utf8), 4);
    }

    #[test]
    fn crlf_is_not_part_of_the_line() {
        assert_eq!(column_to_lsp("abc\r\n", 99, Encoding::Utf16), 3);
        assert_eq!(column_from_lsp("abc\r\n", 99, Encoding::Utf16), 4);
    }

    #[test]
    fn symbol_on_word_boundaries() {
        let line = "let resolver = resolve(resolve_all);";
        assert_eq!(locate_symbol(line, "resolve", None), Some(16));
        assert_eq!(locate_symbol(line, "resolve", Some(2)), None);
        assert_eq!(locate_symbol(line, "resolver", None), Some(5));
    }

    #[test]
    fn second_occurrence() {
        assert_eq!(locate_symbol("f(f(x))", "f", Some(2)), Some(3));
    }

    #[test]
    fn symbol_absent() {
        assert_eq!(locate_symbol("let a = 1;", "b", None), None);
        assert_eq!(locate_symbol("let a = 1;", "", None), None);
    }

    #[test]
    fn symbol_column_counts_scalars() {
        assert_eq!(locate_symbol("日本 name", "name", None), Some(4));
    }

    #[test]
    fn uri_round_trip() {
        let p = std::env::temp_dir().join("a b").join("c.rs");
        let uri = path_to_uri(&p).unwrap();
        assert!(uri.starts_with("file://"));
        assert_eq!(uri_to_path(&uri).unwrap(), p);
    }

    #[cfg(windows)]
    #[test]
    fn windows_drive_path_round_trips() {
        let p = PathBuf::from(r"C:\work\proj\src\lib.rs");
        let uri = path_to_uri(&p).unwrap();
        assert_eq!(uri_to_path(&uri).unwrap(), p);
    }

    fn lines(_: &Path, _: u32) -> Option<String> {
        Some("    pub fn resolve(cwd: &Path)\n".into())
    }

    #[test]
    fn inside_outside_and_vizier() {
        let cwd = std::env::temp_dir().join("proj");
        let inside = path_to_uri(&cwd.join("src/lib.rs")).unwrap();
        let loc = map_location(&cwd, &inside, 11, 11, Encoding::Utf16, &lines).unwrap();
        assert_eq!(
            loc,
            Location::Inside {
                path: "src/lib.rs".into(),
                line: 12,
                character: 12,
                preview: "pub fn resolve(cwd: &Path)".into()
            }
        );

        let outside = path_to_uri(&std::env::temp_dir().join("elsewhere/option.rs")).unwrap();
        let loc = map_location(&cwd, &outside, 570, 0, Encoding::Utf16, &lines).unwrap();
        let text = serde_json::to_string(&loc).unwrap();
        assert!(text.contains("outside_project"));
        assert!(text.contains("option.rs"));
        assert!(!text.contains("elsewhere"));

        let hidden = path_to_uri(&cwd.join(".vizier/agent/wt/a.rs")).unwrap();
        assert!(map_location(&cwd, &hidden, 0, 0, Encoding::Utf16, &lines).is_none());
    }
}
