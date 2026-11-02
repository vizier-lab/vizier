# Contract: BlobStore

Module `crates/vizier-core/src/storage/blob/`.

```rust
pub type ByteStream = Pin<Box<dyn Stream<Item = std::io::Result<Bytes>> + Send>>;

#[async_trait]
pub trait BlobStore: Send + Sync {
    async fn put(&self, key: &BlobKey, bytes: Bytes) -> Result<BlobMeta>;
    async fn put_stream(&self, key: &BlobKey, body: ByteStream, len: Option<u64>) -> Result<BlobMeta>;
    async fn get(&self, key: &BlobKey) -> Result<Option<Bytes>>;
    async fn get_range(&self, key: &BlobKey, range: Option<(u64, u64)>) -> Result<Option<(BlobMeta, ByteStream)>>; // inclusive end
    async fn head(&self, key: &BlobKey) -> Result<Option<BlobMeta>>;
    async fn delete(&self, key: &BlobKey) -> Result<()>;            // idempotent
    async fn delete_prefix(&self, prefix: &BlobKey) -> Result<()>;  // e.g. an agent's artifacts
    async fn list(&self, prefix: &BlobKey) -> Result<Vec<BlobMeta>>;
}
```

`BlobKey::new(&str)` validates segments (non-empty, no `.`/`..`, no `\`, no NUL, ≤ 1024 bytes); `BlobKey::join`.

## Semantics

- `put*` is atomic from a reader's view: local writes `<path>.tmp-<uuid>` then renames; S3 objects appear whole.
- `put*` returns only after the bytes are durable and the size matches; callers write metadata rows **after** `put` returns (FR-017).
- `get_range` streams; memory use is bounded by chunk size (64 KiB local, network chunks for S3).
- `delete` of a missing key succeeds.

## S3BlobStore

- Config: `bucket`, `region`, `endpoint` (optional; default `https://s3.<region>.amazonaws.com`), `path_style` (default true when `endpoint` is set), `prefix` (optional).
- Credentials: `VIZIER_S3_ACCESS_KEY_ID`/`VIZIER_S3_SECRET_ACCESS_KEY`/optional `VIZIER_S3_SESSION_TOKEN`, falling back to `AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY`/`AWS_SESSION_TOKEN`.
- Requests signed with `llm::sigv4` (service `s3`, `x-amz-content-sha256` = payload SHA-256, or `UNSIGNED-PAYLOAD` for streamed bodies over TLS).
- Operations: `PUT`, `GET` (+`Range`), `HEAD`, `DELETE`, `ListObjectsV2` (paged), multipart (`CreateMultipartUpload`/`UploadPart`/`CompleteMultipartUpload`/`AbortMultipartUpload`) for bodies over 64 MiB, 16 MiB parts.
- Startup probe: put/head/get/delete of `.vizier-probe/<uuid>`; failure refuses startup with a credential-free error.
- Errors never include the secret key, session token or signature.

## Consumers

| Consumer | Before | After |
|---|---|---|
| `DocumentStore` (memory) | `LocalDocumentStore` on `tokio::fs` | `BlobDocumentStore` mapping `<agent>/memory/...` → `agents/<agent>/memory/...` |
| `FileManager` uploads | `uploads/<id>/<name>` on disk | `BlobStore` keys; `get` by listing `uploads/<id>/` |
| `ArtifactPaths` | `content.v<n>` on disk | `BlobStore` keys |
| HTTP `/files/{id}`, artifact content | whole read or seek+read | `Body::from_stream(get_range)` |
| `ArtifactStorage::artifact_content_file` | sync path | removed; async `Artifacts::read_content` |
