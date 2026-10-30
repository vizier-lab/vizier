use anyhow::Result;
use std::path::Path;

/// Creates a tar archive from a directory and returns it as bytes
pub fn create_tar_archive(dir: &Path) -> Result<Vec<u8>> {
    let mut tar = tar::Builder::new(Vec::new());

    // Append all contents of the directory to the archive
    tar.append_dir_all(".", dir)?;
    tar.finish()?;

    let archive = tar.into_inner()?;
    Ok(archive)
}
