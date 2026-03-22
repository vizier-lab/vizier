use std::path::PathBuf;

use serde::{Serialize, de::DeserializeOwned};

use crate::error::VizierError;

pub fn read_markdown<T: DeserializeOwned + Clone>(
    path: PathBuf,
) -> Result<(T, String), VizierError> {
    let raw = std::fs::read_to_string(&path).map_err(|err| VizierError(err.to_string()))?;

    let mut content = raw.split('\n').into_iter().collect::<Vec<_>>();

    // naively get frontmatter
    let mut curr = content.remove(0);
    if curr != "---" {
        return VizierError("failed to find frontmatter_raw".into()).into();
    }
    let mut frontmatter_raw = vec![];
    loop {
        curr = content.remove(0);
        if curr == "---" {
            break;
        }

        frontmatter_raw.push(curr);
    }

    let frontmatter = frontmatter_raw.join("\n");
    let frontmatter =
        serde_yaml::from_str::<T>(&frontmatter).map_err(|err| VizierError(err.to_string()))?;

    let content = content.join("\n");

    Ok((frontmatter.clone(), content))
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

    let _ = std::fs::write(path, format!("---\n{}\n---\n{}", frontmatter, content))
        .map_err(|err| VizierError(err.to_string()))?;

    Ok(())
}
