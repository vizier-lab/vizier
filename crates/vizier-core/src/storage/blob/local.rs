//! The default [`BlobStore`]: files under the workspace, `<root>/<key>`, so an existing
//! workspace's uploads, artifacts and memory documents are read in place.

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use super::{BlobKey, BlobMeta, BlobStore, ByteStream};

/// Read chunk size for streamed reads.
const CHUNK: usize = 64 * 1024;

/// Marks an in-flight write; such files are never listed.
const TEMP_MARKER: &str = ".tmp-";

pub struct LocalBlobStore {
    root: PathBuf,
}

/// Drop `.` components: a workspace like `./.vizier` would otherwise keep a `CurDir` component
/// that makes `strip_prefix` against walked paths fail.
fn strip_curdir_components(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        if matches!(component, std::path::Component::CurDir) {
            continue;
        }
        normalized.push(component);
    }
    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

fn not_found(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::NotFound
}

fn modified(meta: &std::fs::Metadata) -> Option<DateTime<Utc>> {
    meta.modified().ok().map(DateTime::<Utc>::from)
}

fn is_temp_name(name: &str) -> bool {
    name.rsplit_once(TEMP_MARKER)
        .is_some_and(|(_, id)| uuid::Uuid::parse_str(id).is_ok())
}

impl LocalBlobStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: strip_curdir_components(root.into()),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn path(&self, key: &BlobKey) -> Result<PathBuf> {
        let mut full = self.root.clone();
        for segment in key.as_str().split('/') {
            // On Windows `\` is a separator and `C:` a drive; a reserved name opens a device.
            if cfg!(windows)
                && let Some(why) = super::windows_segment_problem(segment)
            {
                return Err(anyhow!(
                    "invalid blob key '{key}': segment '{segment}' {why}"
                ));
            }
            full.push(segment);
        }
        Ok(full)
    }

    fn meta(key: &BlobKey, meta: &std::fs::Metadata) -> BlobMeta {
        BlobMeta {
            key: key.clone(),
            size: meta.len(),
            etag: None,
            modified: modified(meta),
        }
    }

    /// Write `body` to a temp file beside `target`, check its length, make it durable and
    /// rename it into place. The temp file is removed on any failure.
    async fn write_atomic(
        &self,
        key: &BlobKey,
        mut body: ByteStream,
        len: Option<u64>,
    ) -> Result<BlobMeta> {
        let target = self.path(key)?;
        let parent = target
            .parent()
            .ok_or_else(|| anyhow!("blob key '{key}' has no parent directory"))?;
        tokio::fs::create_dir_all(parent).await?;
        let file_name = target
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let temp = parent.join(format!("{file_name}{TEMP_MARKER}{}", uuid::Uuid::new_v4()));

        let written: Result<u64> = async {
            let mut file = tokio::fs::File::create(&temp).await?;
            let mut written = 0u64;
            while let Some(chunk) = body.next().await {
                let chunk = chunk?;
                written += chunk.len() as u64;
                file.write_all(&chunk).await?;
            }
            if let Some(expected) = len
                && expected != written
            {
                return Err(anyhow!(
                    "blob '{key}' body was {written} bytes, expected {expected}"
                ));
            }
            file.flush().await?;
            file.sync_all().await?;
            Ok(written)
        }
        .await;

        let result: Result<()> = match written {
            Ok(_) => tokio::fs::rename(&temp, &target).await.map_err(Into::into),
            Err(e) => Err(e),
        };
        if let Err(e) = result {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(e);
        }
        let meta = tokio::fs::metadata(&target).await?;
        Ok(Self::meta(key, &meta))
    }
}

#[async_trait::async_trait]
impl BlobStore for LocalBlobStore {
    async fn put(&self, key: &BlobKey, bytes: Bytes) -> Result<BlobMeta> {
        let len = bytes.len() as u64;
        let body: ByteStream = Box::pin(futures::stream::once(async move { Ok(bytes) }));
        self.write_atomic(key, body, Some(len)).await
    }

    async fn put_stream(
        &self,
        key: &BlobKey,
        body: ByteStream,
        len: Option<u64>,
    ) -> Result<BlobMeta> {
        self.write_atomic(key, body, len).await
    }

    async fn get(&self, key: &BlobKey) -> Result<Option<Bytes>> {
        let path = self.path(key)?;
        match tokio::fs::metadata(&path).await {
            Ok(meta) if meta.is_file() => {}
            Ok(_) => return Ok(None),
            Err(e) if not_found(&e) => return Ok(None),
            Err(e) => return Err(e.into()),
        }
        match tokio::fs::read(&path).await {
            Ok(bytes) => Ok(Some(Bytes::from(bytes))),
            Err(e) if not_found(&e) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    async fn get_range(
        &self,
        key: &BlobKey,
        range: Option<(u64, u64)>,
    ) -> Result<Option<(BlobMeta, ByteStream)>> {
        let path = self.path(key)?;
        let mut file = match tokio::fs::File::open(&path).await {
            Ok(file) => file,
            Err(e) if not_found(&e) => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let meta = file.metadata().await?;
        if !meta.is_file() {
            return Ok(None);
        }
        let size = meta.len();
        let (start, remaining) = match range {
            None => (0, size),
            Some((start, end)) => {
                if start > end || end >= size {
                    return Err(anyhow!(
                        "range {start}-{end} is outside blob '{key}' of {size} bytes"
                    ));
                }
                (start, end - start + 1)
            }
        };
        if start > 0 {
            file.seek(std::io::SeekFrom::Start(start)).await?;
        }
        let stream =
            futures::stream::try_unfold((file, remaining), |(mut file, remaining)| async move {
                if remaining == 0 {
                    return Ok(None);
                }
                let want = remaining.min(CHUNK as u64) as usize;
                let mut buf = vec![0u8; want];
                let read = file.read(&mut buf).await?;
                if read == 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "blob shrank while being read",
                    ));
                }
                buf.truncate(read);
                Ok(Some((Bytes::from(buf), (file, remaining - read as u64))))
            });
        Ok(Some((Self::meta(key, &meta), Box::pin(stream))))
    }

    async fn head(&self, key: &BlobKey) -> Result<Option<BlobMeta>> {
        let path = self.path(key)?;
        match tokio::fs::metadata(&path).await {
            Ok(meta) if meta.is_file() => Ok(Some(Self::meta(key, &meta))),
            Ok(_) => Ok(None),
            Err(e) if not_found(&e) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    async fn delete(&self, key: &BlobKey) -> Result<()> {
        let path = self.path(key)?;
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if not_found(&e) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    async fn delete_prefix(&self, prefix: &BlobKey) -> Result<()> {
        let path = self.path(prefix)?;
        let meta = match tokio::fs::symlink_metadata(&path).await {
            Ok(meta) => meta,
            Err(e) if not_found(&e) => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        let removed = if meta.is_dir() {
            tokio::fs::remove_dir_all(&path).await
        } else {
            tokio::fs::remove_file(&path).await
        };
        match removed {
            Ok(()) => Ok(()),
            Err(e) if not_found(&e) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    async fn list(&self, prefix: &BlobKey) -> Result<Vec<BlobMeta>> {
        let base = self.path(prefix)?;
        let mut out = Vec::new();
        let mut pending = vec![(base, prefix.clone())];
        while let Some((dir, dir_key)) = pending.pop() {
            let mut entries = match tokio::fs::read_dir(&dir).await {
                Ok(entries) => entries,
                Err(e) if not_found(&e) || e.kind() == std::io::ErrorKind::NotADirectory => {
                    continue;
                }
                Err(e) => return Err(e.into()),
            };
            while let Some(entry) = entries.next_entry().await? {
                let name = entry.file_name().to_string_lossy().to_string();
                let Ok(key) = dir_key.join(&name) else {
                    tracing::warn!(dir = %dir_key, "skipping a file whose name is not a valid blob key");
                    continue;
                };
                let meta = match entry.metadata().await {
                    Ok(meta) => meta,
                    Err(e) if not_found(&e) => continue,
                    Err(e) => return Err(e.into()),
                };
                if meta.is_dir() {
                    pending.push((entry.path(), key));
                } else if meta.is_file() && !is_temp_name(&name) {
                    out.push(Self::meta(&key, &meta));
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn conformance() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalBlobStore::new(dir.path());
        super::super::conformance::run(&store).await;
    }

    #[tokio::test]
    async fn keys_map_to_todays_workspace_paths() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalBlobStore::new(dir.path());
        let key = BlobKey::new("uploads/f1/a.txt").unwrap();
        store.put(&key, Bytes::from_static(b"x")).await.unwrap();
        assert_eq!(
            std::fs::read(dir.path().join("uploads").join("f1").join("a.txt")).unwrap(),
            b"x"
        );
        // No temp file is left behind.
        assert_eq!(
            std::fs::read_dir(dir.path().join("uploads/f1"))
                .unwrap()
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn a_root_with_a_curdir_component_lists_relative_keys() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("nested")).unwrap();
        let store =
            LocalBlobStore::new(PathBuf::from(format!("{}/./nested", dir.path().display())));
        store
            .put(&BlobKey::new("p/a.md").unwrap(), Bytes::from_static(b"1"))
            .await
            .unwrap();
        let listed = store.list(&BlobKey::new("p").unwrap()).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].key.as_str(), "p/a.md");
    }

    #[tokio::test]
    async fn temp_files_are_not_listed() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalBlobStore::new(dir.path());
        std::fs::create_dir_all(dir.path().join("p")).unwrap();
        std::fs::write(
            dir.path()
                .join("p")
                .join(format!("x{TEMP_MARKER}{}", uuid::Uuid::new_v4())),
            b"partial",
        )
        .unwrap();
        assert!(
            store
                .list(&BlobKey::new("p").unwrap())
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn an_out_of_bounds_range_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalBlobStore::new(dir.path());
        let key = BlobKey::new("k").unwrap();
        store.put(&key, Bytes::from_static(b"abc")).await.unwrap();
        assert!(store.get_range(&key, Some((1, 3))).await.is_err());
        assert!(store.get_range(&key, Some((2, 1))).await.is_err());
    }
}
