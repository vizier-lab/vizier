//! Where user bytes live (`specs/045-pluggable-storage`, contracts/blob-store.md): uploads,
//! artifact versions and memory documents all go through one [`BlobStore`]. [`LocalBlobStore`]
//! keeps today's on-disk layout under the workspace; [`S3BlobStore`] puts the same keys in a
//! bucket.

mod local;
mod s3;

use crate::time::Timestamp;
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use bytes::Bytes;

use futures::Stream;

pub use local::LocalBlobStore;
pub use s3::{S3BlobStore, S3Config, S3Credentials};

/// Open the configured file store (spec 045): the workspace directory (default) or an
/// S3-compatible bucket, checked with a probe object before anything is stored. Errors name the
/// bucket and the HTTP status or S3 code, never a credential.
pub async fn open(config: &crate::config::VizierConfig) -> Result<std::sync::Arc<dyn BlobStore>> {
    use crate::config::storage::ObjectsConfig;
    Ok(match &config.objects {
        ObjectsConfig::Local => std::sync::Arc::new(LocalBlobStore::new(config.workspace.clone())),
        ObjectsConfig::S3(s3) => {
            let s3 = s3.clone().with_env_overrides();
            if s3.bucket.trim().is_empty() || s3.region.trim().is_empty() {
                return Err(anyhow!(
                    "objects: S3 needs a bucket and a region (objects.bucket/region or VIZIER_S3_BUCKET/VIZIER_S3_REGION)"
                ));
            }
            let credentials = S3Credentials::from_env()
                .map_err(|_| anyhow!("objects: S3 credentials are missing (VIZIER_S3_ACCESS_KEY_ID, VIZIER_S3_SECRET_ACCESS_KEY)"))?;
            let bucket = s3.bucket.clone();
            let store = S3BlobStore::with_credentials(s3, reqwest::Client::new(), credentials)?;
            store.probe().await.map_err(|e| {
                anyhow!(
                    "objects: S3 bucket \"{bucket}\" is not reachable ({})",
                    probe_failure_kind(&e)
                )
            })?;
            std::sync::Arc::new(store)
        }
    })
}

/// Reduce a probe error to a status or I/O class. Raw reqwest/S3 errors can contain the
/// configured endpoint and object key, so they must not cross the startup boundary.
fn probe_failure_kind(error: &anyhow::Error) -> String {
    for cause in error.chain() {
        if let Some(error) = cause.downcast_ref::<s3::S3StatusError>() {
            return error.status.as_u16().to_string();
        }
        if let Some(error) = cause.downcast_ref::<reqwest::Error>() {
            if let Some(status) = error.status() {
                return status.as_u16().to_string();
            }
            if error.is_timeout() {
                return "timed out".into();
            }
            if error.is_connect() {
                return "connection failed".into();
            }
        }
    }
    "connection failed".into()
}

/// A streamed body. Chunks are bounded (64 KiB locally, network chunks for S3).
pub type ByteStream = Pin<Box<dyn Stream<Item = std::io::Result<Bytes>> + Send>>;

/// Longest key accepted, in bytes.
const MAX_KEY_LEN: usize = 1024;

/// A validated, relative, `/`-separated key. Every segment is non-empty and none is `.`, `..`,
/// or contains `\` or NUL, so a key can never name anything outside the store's root.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlobKey(String);

impl BlobKey {
    pub fn new(key: &str) -> Result<Self> {
        if key.is_empty() {
            return Err(anyhow!("blob key is empty"));
        }
        if key.len() > MAX_KEY_LEN {
            return Err(anyhow!("blob key is longer than {MAX_KEY_LEN} bytes"));
        }
        for segment in key.split('/') {
            if let Some(why) = segment_problem(segment) {
                return Err(anyhow!(
                    "invalid blob key '{key}': segment '{segment}' {why}"
                ));
            }
        }
        Ok(Self(key.to_string()))
    }

    /// `self/rest`, where `rest` is itself validated (it may hold several segments).
    pub fn join(&self, rest: &str) -> Result<Self> {
        Self::new(&format!("{}/{rest}", self.0))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The last segment.
    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(&self.0)
    }

    /// The part of `self` after `prefix/`, if `self` is under `prefix`.
    pub fn strip_prefix(&self, prefix: &BlobKey) -> Option<&str> {
        self.0
            .strip_prefix(prefix.as_str())
            .and_then(|rest| rest.strip_prefix('/'))
    }
}

impl fmt::Display for BlobKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn segment_problem(segment: &str) -> Option<&'static str> {
    if segment.is_empty() {
        return Some("is empty");
    }
    if segment == "." || segment == ".." {
        return Some("is a relative path component");
    }
    if segment.contains('\\') {
        return Some("contains a backslash");
    }
    if segment.contains('\0') {
        return Some("contains NUL");
    }
    None
}

/// Why `segment` cannot be one file or directory name on Windows, if it cannot. Pure so it is
/// tested on every OS; `LocalBlobStore` applies it only where it matters.
pub(crate) fn windows_segment_problem(segment: &str) -> Option<&'static str> {
    if segment
        .chars()
        .any(|c| c.is_control() || "<>:\"\\|?*".contains(c))
    {
        return Some("contains a character Windows does not allow in names");
    }
    if segment.ends_with('.') || segment.ends_with(' ') {
        return Some("ends with a dot or a space");
    }
    let stem = segment
        .split('.')
        .next()
        .unwrap_or(segment)
        .to_ascii_lowercase();
    let reserved = matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || ["com", "lpt"].iter().any(|p| {
            stem.strip_prefix(p)
                .is_some_and(|n| matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
        });
    reserved.then_some("is a reserved device name on Windows")
}

/// What a store knows about one object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobMeta {
    pub key: BlobKey,
    pub size: u64,
    pub etag: Option<String>,
    pub modified: Option<Timestamp>,
}

/// The one byte store (contracts/blob-store.md).
#[async_trait::async_trait]
pub trait BlobStore: Send + Sync {
    /// Store `bytes` at `key`, replacing any object there. Readers see the old or the new
    /// object, never a partial one. Returns after the bytes are durable.
    async fn put(&self, key: &BlobKey, bytes: Bytes) -> Result<BlobMeta>;

    /// [`BlobStore::put`] from a stream. `len`, when given, must match what the stream yields.
    async fn put_stream(
        &self,
        key: &BlobKey,
        body: ByteStream,
        len: Option<u64>,
    ) -> Result<BlobMeta>;

    /// The whole object, `None` when missing.
    async fn get(&self, key: &BlobKey) -> Result<Option<Bytes>>;

    /// The object (or the inclusive byte range `start..=end` of it) as a stream, with its
    /// metadata (whole-object size). `None` when missing.
    async fn get_range(
        &self,
        key: &BlobKey,
        range: Option<(u64, u64)>,
    ) -> Result<Option<(BlobMeta, ByteStream)>>;

    async fn head(&self, key: &BlobKey) -> Result<Option<BlobMeta>>;

    /// Remove `key`. Missing is not an error.
    async fn delete(&self, key: &BlobKey) -> Result<()>;

    /// Remove every object under `prefix/` (and `prefix` itself). Missing is not an error.
    async fn delete_prefix(&self, prefix: &BlobKey) -> Result<()>;

    /// Every object under `prefix/`, recursively, in no particular order.
    async fn list(&self, prefix: &BlobKey) -> Result<Vec<BlobMeta>>;
}

/// A located object plus the store it is in, so an adapter can stream it (optionally a range)
/// without knowing which backend holds it.
#[derive(Clone)]
pub struct BlobRef {
    pub store: Arc<dyn BlobStore>,
    pub meta: BlobMeta,
}

impl BlobRef {
    /// Stream the object, or the inclusive range `start..=end`. `None` when it vanished since
    /// it was located.
    pub async fn open(&self, range: Option<(u64, u64)>) -> Result<Option<ByteStream>> {
        Ok(self
            .store
            .get_range(&self.meta.key, range)
            .await?
            .map(|(_, stream)| stream))
    }
}

#[cfg(test)]
mod key_tests {
    use super::*;

    #[test]
    fn accepts_relative_keys_and_joins() {
        let key = BlobKey::new("uploads/abc/file name.txt").unwrap();
        assert_eq!(key.file_name(), "file name.txt");
        let base = BlobKey::new("artifacts/a").unwrap();
        let joined = base.join("id/content.v1").unwrap();
        assert_eq!(joined.as_str(), "artifacts/a/id/content.v1");
        assert_eq!(joined.strip_prefix(&base), Some("id/content.v1"));
        assert_eq!(
            BlobKey::new("artifacts/ab").unwrap().strip_prefix(&base),
            None
        );
    }

    #[test]
    fn rejects_traversal_and_malformed_keys() {
        for bad in [
            "", "/a", "a/", "a//b", ".", "..", "a/../b", "a/./b", "a\\b", "a\0b",
        ] {
            assert!(BlobKey::new(bad).is_err(), "{bad:?} should be refused");
        }
        assert!(BlobKey::new(&"x".repeat(MAX_KEY_LEN + 1)).is_err());
        assert!(BlobKey::new("a").unwrap().join("../b").is_err());
    }

    #[test]
    fn windows_rejects_what_windows_cannot_name_or_would_escape_with() {
        for bad in [
            "a:b", "..\\x", "C:", "a|b", "a?", "con", "NUL.txt", "com1", "lpt9.md", "x.", "x ",
            "a\u{1}b",
        ] {
            assert!(
                windows_segment_problem(bad).is_some(),
                "{bad:?} should be refused"
            );
        }
    }

    #[test]
    fn windows_accepts_ordinary_names() {
        for ok in [
            "notes.md",
            "index.md",
            "my-bundle",
            "コンテキスト.md",
            "console",
            "com10",
            "a b.md",
        ] {
            assert!(
                windows_segment_problem(ok).is_none(),
                "{ok:?} should be fine"
            );
        }
    }
}

/// The behaviour every store must have; run against local always and S3 when configured.
#[cfg(test)]
pub(crate) mod conformance {
    use super::*;
    use futures::StreamExt;

    pub async fn collect(mut stream: ByteStream) -> Vec<u8> {
        let mut out = Vec::new();
        while let Some(chunk) = stream.next().await {
            out.extend_from_slice(&chunk.unwrap());
        }
        out
    }

    fn key(s: &str) -> BlobKey {
        BlobKey::new(s).unwrap()
    }

    pub async fn run(store: &dyn BlobStore) {
        // put / get / head
        let k = key("t/a/one.txt");
        assert!(store.get(&k).await.unwrap().is_none());
        assert!(store.head(&k).await.unwrap().is_none());
        let meta = store
            .put(&k, Bytes::from_static(b"hello world"))
            .await
            .unwrap();
        assert_eq!(meta.size, 11);
        assert_eq!(meta.key, k);
        assert_eq!(
            store.get(&k).await.unwrap().unwrap().as_ref(),
            b"hello world"
        );
        assert_eq!(store.head(&k).await.unwrap().unwrap().size, 11);

        // overwrite
        store
            .put(&k, Bytes::from_static(b"HELLO world!"))
            .await
            .unwrap();
        assert_eq!(
            store.get(&k).await.unwrap().unwrap().as_ref(),
            b"HELLO world!"
        );

        // range
        let (meta, stream) = store.get_range(&k, Some((2, 6))).await.unwrap().unwrap();
        assert_eq!(meta.size, 12);
        assert_eq!(collect(stream).await, b"LLO w");
        let (_, stream) = store.get_range(&k, None).await.unwrap().unwrap();
        assert_eq!(collect(stream).await, b"HELLO world!");
        assert!(
            store
                .get_range(&key("t/missing"), None)
                .await
                .unwrap()
                .is_none()
        );

        // put_stream, larger than one local chunk
        let big: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let chunks: Vec<std::io::Result<Bytes>> = big
            .chunks(10_000)
            .map(|c| Ok(Bytes::copy_from_slice(c)))
            .collect();
        let body: ByteStream = Box::pin(futures::stream::iter(chunks));
        let k2 = key("t/a/nested/big.bin");
        let meta = store
            .put_stream(&k2, body, Some(big.len() as u64))
            .await
            .unwrap();
        assert_eq!(meta.size, big.len() as u64);
        assert_eq!(store.get(&k2).await.unwrap().unwrap().as_ref(), &big[..]);
        let (_, stream) = store
            .get_range(&k2, Some((65_530, 140_000)))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(collect(stream).await, &big[65_530..=140_000]);

        // a wrong declared length is refused and leaves nothing behind
        let body: ByteStream =
            Box::pin(futures::stream::iter(vec![Ok(Bytes::from_static(b"abc"))]));
        let k3 = key("t/a/short.bin");
        assert!(store.put_stream(&k3, body, Some(10)).await.is_err());
        assert!(store.head(&k3).await.unwrap().is_none());

        // list
        store
            .put(&key("t/b/two.txt"), Bytes::from_static(b"2"))
            .await
            .unwrap();
        let mut listed: Vec<String> = store
            .list(&key("t/a"))
            .await
            .unwrap()
            .into_iter()
            .map(|m| m.key.as_str().to_string())
            .collect();
        listed.sort();
        assert_eq!(listed, vec!["t/a/nested/big.bin", "t/a/one.txt"]);
        assert!(store.list(&key("t/none")).await.unwrap().is_empty());
        // `t/a` must not match a sibling sharing the prefix string
        store
            .put(&key("t/ab"), Bytes::from_static(b"x"))
            .await
            .unwrap();
        assert_eq!(store.list(&key("t/a")).await.unwrap().len(), 2);

        // delete (idempotent) and delete_prefix
        store.delete(&k).await.unwrap();
        store.delete(&k).await.unwrap();
        assert!(store.get(&k).await.unwrap().is_none());
        store.delete_prefix(&key("t/a")).await.unwrap();
        store.delete_prefix(&key("t/a")).await.unwrap();
        assert!(store.list(&key("t/a")).await.unwrap().is_empty());
        assert!(store.head(&key("t/b/two.txt")).await.unwrap().is_some());
        assert!(store.head(&key("t/ab")).await.unwrap().is_some());
        store.delete_prefix(&key("t")).await.unwrap();
        assert!(store.list(&key("t")).await.unwrap().is_empty());
    }
}
