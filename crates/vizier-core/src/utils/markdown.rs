use std::path::PathBuf;

use serde::{de::DeserializeOwned, Serialize};

pub use vizier_derive::MarkdownDoc;

use crate::error::VizierError;

pub fn read_content(path: PathBuf) -> Result<String, VizierError> {
    if let Ok((_, res)) = read_markdown::<serde_yaml::Value>(path.clone()) {
        return Ok(res);
    }

    Ok(std::fs::read_to_string(path).map_err(|err| VizierError(err.to_string()))?)
}

pub fn read_markdown<T: DeserializeOwned + Clone>(
    path: PathBuf,
) -> Result<(T, String), VizierError> {
    let raw = std::fs::read_to_string(&path).map_err(|err| VizierError(err.to_string()))?;
    if raw.split(['\n', '\r']).next() != Some("---") {
        return VizierError(format!(
            "failed to find frontmatter for {}",
            path.display()
        ))
        .into();
    }

    parse_markdown_str::<T>(&raw)
}

/// Split `---\n<yaml>\n---\n<body>` and parse the YAML frontmatter as `T`.
///
/// Lines are split on `\n` and `\r`; the first line must be exactly `---`, the YAML runs until
/// the next exact `---` line, and the body is the remaining lines joined with `\n`.
/// Errs when the first line isn't `---`, the block is unterminated, or the YAML doesn't parse as `T`.
pub fn parse_markdown_str<T: DeserializeOwned>(raw: &str) -> Result<(T, String), VizierError> {
    let mut lines = raw.split(['\n', '\r']);
    if lines.next() != Some("---") {
        return Err(VizierError("missing frontmatter".into()));
    }

    let mut frontmatter_raw = vec![];
    loop {
        match lines.next() {
            Some("---") => break,
            Some(line) => frontmatter_raw.push(line),
            None => return Err(VizierError("unterminated frontmatter".into())),
        }
    }

    let frontmatter = serde_yaml::from_str::<T>(&frontmatter_raw.join("\n"))
        .map_err(|err| VizierError(err.to_string()))?;
    let body = lines.collect::<Vec<_>>().join("\n");

    Ok((frontmatter, body))
}

pub fn write_markdown<T: Serialize>(
    frontmatter: &T,
    content: String,
    path: PathBuf,
) -> Result<(), VizierError> {
    let parent = path.parent().unwrap();
    if !parent.exists() {
        let _ = std::fs::create_dir_all(parent);
    }

    let frontmatter =
        serde_yaml::to_string(frontmatter).map_err(|err| VizierError(err.to_string()))?;

    let _ = std::fs::write(path, format!("---\n{}---\n{}", frontmatter, content))
        .map_err(|err| VizierError(err.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &str) -> Result<(serde_yaml::Value, String), VizierError> {
        parse_markdown_str::<serde_yaml::Value>(raw)
    }

    #[test]
    fn splits_frontmatter_and_body() {
        let (fm, body) = parse("---\ntitle: a\n---\nhello\nworld").unwrap();
        assert_eq!(fm["title"], serde_yaml::Value::from("a"));
        assert_eq!(body, "hello\nworld");
    }

    #[test]
    fn empty_frontmatter_is_null() {
        let (fm, body) = parse("---\n---\nbody").unwrap();
        assert_eq!(fm, serde_yaml::Value::Null);
        assert_eq!(body, "body");
    }

    #[test]
    fn crlf_keeps_the_empty_segments_of_the_old_loop() {
        let (fm, body) = parse("---\r\ntitle: a\r\n---\r\nhello").unwrap();
        assert_eq!(fm["title"], serde_yaml::Value::from("a"));
        assert_eq!(body, "\nhello");
    }

    #[test]
    fn missing_header_is_an_error() {
        assert!(parse("no header").is_err());
    }

    #[test]
    fn unclosed_header_is_an_error_not_a_panic() {
        assert!(parse("---\ntitle: a\nhello").is_err());
    }
}
