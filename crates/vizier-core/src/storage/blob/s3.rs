//! [`BlobStore`] on S3 or an S3-compatible service (MinIO, R2, …), over the shared `reqwest`
//! client and the SigV4 signer in [`crate::llm::sigv4`] (research R8).
//!
//! Errors carry the operation, the object key and S3's error code/message — never the secret
//! key, the session token, the `Authorization` header or the signature.

use std::fmt;

use anyhow::{Context, Result, anyhow};
use bytes::{Bytes, BytesMut};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use reqwest::{Method, StatusCode};

use super::{BlobKey, BlobMeta, BlobStore, ByteStream};
use crate::llm::sigv4::{self, SigV4Request, SignOptions};

/// Bodies larger than this go up as a multipart upload.
const MULTIPART_THRESHOLD: usize = 64 * 1024 * 1024;
/// Multipart part size.
const PART_SIZE: usize = 16 * 1024 * 1024;

/// Where the bucket is: the `objects:` configuration.
pub use crate::config::storage::S3Config;

/// S3 credentials. `Debug` never prints the secret or the token.
#[derive(Clone)]
pub struct S3Credentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: Option<String>,
}

impl fmt::Debug for S3Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("S3Credentials")
            .field("access_key_id", &"<redacted>")
            .field("secret_access_key", &"<redacted>")
            .field(
                "session_token",
                &self.session_token.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl S3Credentials {
    /// `VIZIER_S3_ACCESS_KEY_ID`/`VIZIER_S3_SECRET_ACCESS_KEY`/`VIZIER_S3_SESSION_TOKEN`, then
    /// `AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY`/`AWS_SESSION_TOKEN`.
    pub fn from_env() -> Result<Self> {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
        for (id, secret, token) in [
            (
                "VIZIER_S3_ACCESS_KEY_ID",
                "VIZIER_S3_SECRET_ACCESS_KEY",
                "VIZIER_S3_SESSION_TOKEN",
            ),
            (
                "AWS_ACCESS_KEY_ID",
                "AWS_SECRET_ACCESS_KEY",
                "AWS_SESSION_TOKEN",
            ),
        ] {
            if let (Some(access_key_id), Some(secret_access_key)) = (var(id), var(secret)) {
                return Ok(Self {
                    access_key_id,
                    secret_access_key,
                    session_token: var(token),
                });
            }
        }
        Err(anyhow!(
            "no S3 credentials: set VIZIER_S3_ACCESS_KEY_ID and VIZIER_S3_SECRET_ACCESS_KEY (or AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY)"
        ))
    }
}

pub struct S3BlobStore {
    http: reqwest::Client,
    scheme: String,
    /// `host[:port]` requests go to (and are signed for).
    host: String,
    path_style: bool,
    bucket: String,
    region: String,
    prefix: Option<String>,
    credentials: S3Credentials,
    multipart_threshold: usize,
    part_size: usize,
}

impl fmt::Debug for S3BlobStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("S3BlobStore")
            .field("host", &self.host)
            .field("bucket", &self.bucket)
            .field("region", &self.region)
            .field("prefix", &self.prefix)
            .finish_non_exhaustive()
    }
}

/// S3's URI encoding: unreserved bytes pass, everything else is `%XX`; `/` passes only when
/// `keep_slash` (object paths), never in query values.
fn uri_encode(value: &str, keep_slash: bool) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            b'/' if keep_slash => out.push('/'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The text of the first `<tag>…</tag>` in `xml`, unescaped.
fn xml_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml_unescape(&xml[start..end]))
}

/// Every `<tag>…</tag>` block's inner text.
fn xml_blocks<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find(&open) {
        let body = &rest[start + open.len()..];
        let Some(end) = body.find(&close) else { break };
        out.push(&body[..end]);
        rest = &body[end + close.len()..];
    }
    out
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn transport_error(op: &str, key: &str, error: reqwest::Error) -> anyhow::Error {
    anyhow::Error::new(error.without_url()).context(format!("S3 {op} '{key}' failed"))
}

#[derive(Debug)]
pub(super) struct S3StatusError {
    pub(super) status: StatusCode,
    operation: String,
    detail: String,
}

impl fmt::Display for S3StatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}{}", self.operation, self.status, self.detail)
    }
}

impl std::error::Error for S3StatusError {}

/// A credential-free error from a non-success response: status plus S3's `Code`/`Message`.
async fn status_error(op: &str, key: &str, response: reqwest::Response) -> anyhow::Error {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let detail = match (xml_tag(&body, "Code"), xml_tag(&body, "Message")) {
        (Some(code), Some(message)) => format!(" {code}: {message}"),
        (Some(code), None) => format!(" {code}"),
        _ => String::new(),
    };
    S3StatusError {
        status,
        operation: format!("S3 {op} '{key}' failed"),
        detail,
    }
    .into()
}

fn header_str<'a>(response: &'a reqwest::Response, name: &str) -> Option<&'a str> {
    response.headers().get(name).and_then(|v| v.to_str().ok())
}

fn parse_http_date(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc2822(value)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

impl S3BlobStore {
    /// A store with credentials from the environment ([`S3Credentials::from_env`]).
    pub fn new(config: S3Config, http: reqwest::Client) -> Result<Self> {
        Self::with_credentials(config, http, S3Credentials::from_env()?)
    }

    pub fn with_credentials(
        config: S3Config,
        http: reqwest::Client,
        credentials: S3Credentials,
    ) -> Result<Self> {
        if config.bucket.is_empty() {
            return Err(anyhow!("S3 bucket name is empty"));
        }
        let endpoint = config
            .endpoint
            .clone()
            .unwrap_or_else(|| format!("https://s3.{}.amazonaws.com", config.region));
        let url = reqwest::Url::parse(&endpoint)
            .map_err(|e| anyhow!("invalid S3 endpoint '{endpoint}': {e}"))?;
        let host_name = url
            .host_str()
            .ok_or_else(|| anyhow!("S3 endpoint '{endpoint}' has no host"))?;
        let host = match url.port() {
            Some(port) => format!("{host_name}:{port}"),
            None => host_name.to_string(),
        };
        let path_style = config.path_style.unwrap_or(config.endpoint.is_some());
        let host = if path_style {
            host
        } else {
            format!("{}.{host}", config.bucket)
        };
        let prefix = config
            .prefix
            .as_deref()
            .map(|p| p.trim_matches('/'))
            .filter(|p| !p.is_empty())
            .map(|p| BlobKey::new(p).map(|k| k.as_str().to_string()))
            .transpose()?;
        Ok(Self {
            http,
            scheme: url.scheme().to_string(),
            host,
            path_style,
            bucket: config.bucket,
            region: config.region,
            prefix,
            credentials,
            multipart_threshold: MULTIPART_THRESHOLD,
            part_size: PART_SIZE,
        })
    }

    /// Startup check: put, head, get and delete `.vizier-probe/<uuid>`. Any failure is a
    /// credential-free error that should refuse startup.
    pub async fn probe(&self) -> Result<()> {
        let key = BlobKey::new(&format!(".vizier-probe/{}", uuid::Uuid::new_v4()))?;
        let payload = Bytes::from_static(b"vizier storage probe");
        let result: Result<()> = async {
            self.put(&key, payload.clone()).await?;
            let meta = self
                .head(&key)
                .await?
                .ok_or_else(|| anyhow!("probe object vanished after it was written"))?;
            if meta.size != payload.len() as u64 {
                return Err(anyhow!("probe object has the wrong size"));
            }
            let read = self
                .get(&key)
                .await?
                .ok_or_else(|| anyhow!("probe object could not be read back"))?;
            if read != payload {
                return Err(anyhow!("probe object read back different bytes"));
            }
            self.delete(&key).await
        }
        .await;
        result.with_context(|| {
            format!(
                "object storage check failed for bucket '{}' at {}",
                self.bucket, self.host
            )
        })
    }

    /// The object name in the bucket for `key`.
    fn object(&self, key: &BlobKey) -> String {
        match &self.prefix {
            Some(prefix) => format!("{prefix}/{key}"),
            None => key.to_string(),
        }
    }

    /// The request path for `object` (or the bucket itself when `None`).
    fn path(&self, object: Option<&str>) -> String {
        match (self.path_style, object) {
            (true, Some(object)) => format!("/{}/{}", self.bucket, uri_encode(object, true)),
            (true, None) => format!("/{}", self.bucket),
            (false, Some(object)) => format!("/{}", uri_encode(object, true)),
            (false, None) => "/".to_string(),
        }
    }

    /// Sign and send one request. `query` values are raw; they are encoded and sorted here.
    async fn send(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Bytes,
        extra_headers: &[(&str, &str)],
    ) -> std::result::Result<reqwest::Response, reqwest::Error> {
        let mut pairs: Vec<String> = query
            .iter()
            .map(|(k, v)| format!("{}={}", uri_encode(k, false), uri_encode(v, false)))
            .collect();
        pairs.sort_unstable();
        let query = pairs.join("&");

        let payload_hash = sigv4::sha256_hex(&body);
        let mut signed: Vec<(&str, &str)> = vec![("x-amz-content-sha256", payload_hash.as_str())];
        signed.extend_from_slice(extra_headers);
        let signature = sigv4::sign_with(
            &SigV4Request {
                method: method.as_str(),
                path,
                query: &query,
                host: &self.host,
                content_type: None,
                body: &[],
                access_key_id: &self.credentials.access_key_id,
                secret_access_key: &self.credentials.secret_access_key,
                session_token: self.credentials.session_token.as_deref(),
                region: &self.region,
                service: "s3",
                now: Utc::now(),
            },
            &SignOptions {
                extra_headers: &signed,
                payload_hash: Some(&payload_hash),
                double_encode_path: false,
            },
        );

        let url = if query.is_empty() {
            format!("{}://{}{path}", self.scheme, self.host)
        } else {
            format!("{}://{}{path}?{query}", self.scheme, self.host)
        };
        let mut request = self
            .http
            .request(method, url)
            .header("x-amz-date", signature.amz_date)
            .header(reqwest::header::AUTHORIZATION, signature.authorization)
            .header("x-amz-content-sha256", payload_hash.as_str());
        if let Some(token) = &self.credentials.session_token {
            request = request.header("x-amz-security-token", token);
        }
        for (name, value) in extra_headers {
            request = request.header(*name, *value);
        }
        request.body(body).send().await
    }

    async fn put_single(&self, key: &BlobKey, bytes: Bytes) -> Result<BlobMeta> {
        let object = self.object(key);
        let size = bytes.len() as u64;
        let response = self
            .send(Method::PUT, &self.path(Some(&object)), &[], bytes, &[])
            .await
            .map_err(|e| transport_error("put", key.as_str(), e))?;
        if !response.status().is_success() {
            return Err(status_error("put", key.as_str(), response).await);
        }
        Ok(BlobMeta {
            key: key.clone(),
            size,
            etag: header_str(&response, "etag").map(str::to_string),
            modified: Some(Utc::now()),
        })
    }

    async fn create_multipart(&self, key: &BlobKey) -> Result<String> {
        let path = self.path(Some(&self.object(key)));
        let response = self
            .send(Method::POST, &path, &[("uploads", "")], Bytes::new(), &[])
            .await
            .map_err(|e| transport_error("create multipart upload", key.as_str(), e))?;
        if !response.status().is_success() {
            return Err(status_error("create multipart upload", key.as_str(), response).await);
        }
        let body = response
            .text()
            .await
            .map_err(|e| transport_error("create multipart upload", key.as_str(), e))?;
        xml_tag(&body, "UploadId")
            .ok_or_else(|| anyhow!("S3 create multipart upload '{key}' returned no upload id"))
    }

    async fn upload_part(
        &self,
        key: &BlobKey,
        upload_id: &str,
        number: usize,
        bytes: Bytes,
    ) -> Result<String> {
        let path = self.path(Some(&self.object(key)));
        let number = number.to_string();
        let response = self
            .send(
                Method::PUT,
                &path,
                &[("partNumber", number.as_str()), ("uploadId", upload_id)],
                bytes,
                &[],
            )
            .await
            .map_err(|e| transport_error("upload part", key.as_str(), e))?;
        if !response.status().is_success() {
            return Err(status_error("upload part", key.as_str(), response).await);
        }
        header_str(&response, "etag")
            .map(str::to_string)
            .ok_or_else(|| anyhow!("S3 upload part '{key}' returned no ETag"))
    }

    async fn complete_multipart(
        &self,
        key: &BlobKey,
        upload_id: &str,
        etags: &[String],
    ) -> Result<Option<String>> {
        let mut xml = String::from("<CompleteMultipartUpload>");
        for (i, etag) in etags.iter().enumerate() {
            xml.push_str(&format!(
                "<Part><PartNumber>{}</PartNumber><ETag>{}</ETag></Part>",
                i + 1,
                xml_escape(etag)
            ));
        }
        xml.push_str("</CompleteMultipartUpload>");
        let path = self.path(Some(&self.object(key)));
        let response = self
            .send(
                Method::POST,
                &path,
                &[("uploadId", upload_id)],
                Bytes::from(xml),
                &[],
            )
            .await
            .map_err(|e| transport_error("complete multipart upload", key.as_str(), e))?;
        if !response.status().is_success() {
            return Err(status_error("complete multipart upload", key.as_str(), response).await);
        }
        // S3 can answer 200 with an `<Error>` body here.
        let body = response
            .text()
            .await
            .map_err(|e| transport_error("complete multipart upload", key.as_str(), e))?;
        if body.contains("<Error>") {
            let code = xml_tag(&body, "Code").unwrap_or_default();
            let message = xml_tag(&body, "Message").unwrap_or_default();
            return Err(anyhow!(
                "S3 complete multipart upload '{key}' failed: {code}: {message}"
            ));
        }
        Ok(xml_tag(&body, "ETag"))
    }

    async fn abort_multipart(&self, key: &BlobKey, upload_id: &str) {
        let path = self.path(Some(&self.object(key)));
        match self
            .send(
                Method::DELETE,
                &path,
                &[("uploadId", upload_id)],
                Bytes::new(),
                &[],
            )
            .await
        {
            Ok(r) if r.status().is_success() || r.status() == StatusCode::NOT_FOUND => {}
            Ok(r) => {
                tracing::warn!(key = %key, status = %r.status(), "failed to abort an S3 multipart upload")
            }
            Err(e) => {
                tracing::warn!(key = %key, "failed to abort an S3 multipart upload: {}", e.without_url())
            }
        }
    }

    /// Multipart upload: `first` (already buffered, ≥ one part) then the rest of `body`.
    async fn put_multipart(
        &self,
        key: &BlobKey,
        first: BytesMut,
        mut body: Option<ByteStream>,
        len: Option<u64>,
    ) -> Result<BlobMeta> {
        let upload_id = self.create_multipart(key).await?;
        let result: Result<(u64, Option<String>)> = async {
            let mut buffer = first;
            let mut etags = Vec::new();
            let mut total = 0u64;
            loop {
                while buffer.len() >= self.part_size {
                    let part = buffer.split_to(self.part_size).freeze();
                    total += part.len() as u64;
                    etags.push(
                        self.upload_part(key, &upload_id, etags.len() + 1, part)
                            .await?,
                    );
                }
                let next = match body.as_mut() {
                    Some(stream) => stream.next().await,
                    None => None,
                };
                match next {
                    Some(chunk) => buffer.extend_from_slice(&chunk?),
                    None => break,
                }
            }
            if !buffer.is_empty() || etags.is_empty() {
                let part = buffer.freeze();
                total += part.len() as u64;
                etags.push(
                    self.upload_part(key, &upload_id, etags.len() + 1, part)
                        .await?,
                );
            }
            if let Some(expected) = len
                && expected != total
            {
                return Err(anyhow!(
                    "blob '{key}' body was {total} bytes, expected {expected}"
                ));
            }
            let etag = self.complete_multipart(key, &upload_id, &etags).await?;
            Ok((total, etag))
        }
        .await;
        match result {
            Ok((size, etag)) => Ok(BlobMeta {
                key: key.clone(),
                size,
                etag,
                modified: Some(Utc::now()),
            }),
            Err(e) => {
                self.abort_multipart(key, &upload_id).await;
                Err(e)
            }
        }
    }

    /// One ListObjectsV2 page under `object_prefix`.
    async fn list_page(
        &self,
        prefix: &BlobKey,
        object_prefix: &str,
        token: Option<&str>,
    ) -> Result<(Vec<BlobMeta>, Option<String>)> {
        let mut query: Vec<(&str, &str)> = vec![("list-type", "2"), ("prefix", object_prefix)];
        if let Some(token) = token {
            query.push(("continuation-token", token));
        }
        let response = self
            .send(Method::GET, &self.path(None), &query, Bytes::new(), &[])
            .await
            .map_err(|e| transport_error("list", prefix.as_str(), e))?;
        if !response.status().is_success() {
            return Err(status_error("list", prefix.as_str(), response).await);
        }
        let body = response
            .text()
            .await
            .map_err(|e| transport_error("list", prefix.as_str(), e))?;
        let mut items = Vec::new();
        for block in xml_blocks(&body, "Contents") {
            let Some(object) = xml_tag(block, "Key") else {
                continue;
            };
            let relative = match &self.prefix {
                Some(p) => object
                    .strip_prefix(p.as_str())
                    .and_then(|rest| rest.strip_prefix('/')),
                None => Some(object.as_str()),
            };
            let Some(key) = relative.and_then(|r| BlobKey::new(r).ok()) else {
                tracing::warn!(prefix = %prefix, "skipping an S3 object whose name is not a valid blob key");
                continue;
            };
            items.push(BlobMeta {
                key,
                size: xml_tag(block, "Size")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                etag: xml_tag(block, "ETag"),
                modified: xml_tag(block, "LastModified")
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|d| d.with_timezone(&Utc)),
            });
        }
        let truncated = xml_tag(&body, "IsTruncated").is_some_and(|t| t == "true");
        let next = if truncated {
            xml_tag(&body, "NextContinuationToken")
        } else {
            None
        };
        Ok((items, next))
    }
}

#[async_trait::async_trait]
impl BlobStore for S3BlobStore {
    async fn put(&self, key: &BlobKey, bytes: Bytes) -> Result<BlobMeta> {
        if bytes.len() > self.multipart_threshold {
            let len = bytes.len() as u64;
            return self
                .put_multipart(key, BytesMut::from(&bytes[..]), None, Some(len))
                .await;
        }
        self.put_single(key, bytes).await
    }

    async fn put_stream(
        &self,
        key: &BlobKey,
        mut body: ByteStream,
        len: Option<u64>,
    ) -> Result<BlobMeta> {
        // Buffer up to the multipart threshold; a body that fits goes up in one PUT.
        let mut buffer = BytesMut::new();
        while buffer.len() <= self.multipart_threshold {
            match body.next().await {
                Some(chunk) => buffer.extend_from_slice(&chunk?),
                None => {
                    if let Some(expected) = len
                        && expected != buffer.len() as u64
                    {
                        return Err(anyhow!(
                            "blob '{key}' body was {} bytes, expected {expected}",
                            buffer.len()
                        ));
                    }
                    return self.put_single(key, buffer.freeze()).await;
                }
            }
        }
        self.put_multipart(key, buffer, Some(body), len).await
    }

    async fn get(&self, key: &BlobKey) -> Result<Option<Bytes>> {
        let path = self.path(Some(&self.object(key)));
        let response = self
            .send(Method::GET, &path, &[], Bytes::new(), &[])
            .await
            .map_err(|e| transport_error("get", key.as_str(), e))?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(status_error("get", key.as_str(), response).await);
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|e| transport_error("get", key.as_str(), e))?;
        Ok(Some(bytes))
    }

    async fn get_range(
        &self,
        key: &BlobKey,
        range: Option<(u64, u64)>,
    ) -> Result<Option<(BlobMeta, ByteStream)>> {
        if let Some((start, end)) = range
            && start > end
        {
            return Err(anyhow!("range {start}-{end} is empty for blob '{key}'"));
        }
        let path = self.path(Some(&self.object(key)));
        let range_header = range.map(|(start, end)| format!("bytes={start}-{end}"));
        let extra: Vec<(&str, &str)> = range_header
            .as_deref()
            .map(|r| vec![("range", r)])
            .unwrap_or_default();
        let response = self
            .send(Method::GET, &path, &[], Bytes::new(), &extra)
            .await
            .map_err(|e| transport_error("get", key.as_str(), e))?;
        let status = response.status();
        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(status_error("get", key.as_str(), response).await);
        }
        let size = if status == StatusCode::PARTIAL_CONTENT {
            header_str(&response, "content-range")
                .and_then(|v| v.rsplit_once('/'))
                .and_then(|(_, total)| total.parse::<u64>().ok())
        } else {
            response.content_length()
        }
        .ok_or_else(|| anyhow!("S3 get '{key}' did not report the object size"))?;
        if let Some((_, end)) = range
            && (end >= size || status != StatusCode::PARTIAL_CONTENT)
        {
            return Err(anyhow!(
                "range ending at {end} is outside blob '{key}' of {size} bytes"
            ));
        }
        let meta = BlobMeta {
            key: key.clone(),
            size,
            etag: header_str(&response, "etag").map(str::to_string),
            modified: header_str(&response, "last-modified").and_then(parse_http_date),
        };
        let stream = futures::stream::try_unfold(response, |mut response| async move {
            match response.chunk().await {
                Ok(Some(chunk)) => Ok(Some((chunk, response))),
                Ok(None) => Ok(None),
                Err(e) => Err(std::io::Error::other(e.without_url())),
            }
        });
        Ok(Some((meta, Box::pin(stream))))
    }

    async fn head(&self, key: &BlobKey) -> Result<Option<BlobMeta>> {
        let path = self.path(Some(&self.object(key)));
        let response = self
            .send(Method::HEAD, &path, &[], Bytes::new(), &[])
            .await
            .map_err(|e| transport_error("head", key.as_str(), e))?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(anyhow!("S3 head '{key}' failed: {}", response.status()));
        }
        // `content_length()` is the body's size, which HEAD does not have; read the header.
        let size = header_str(&response, "content-length")
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| anyhow!("S3 head '{key}' did not report the object size"))?;
        Ok(Some(BlobMeta {
            key: key.clone(),
            size,
            etag: header_str(&response, "etag").map(str::to_string),
            modified: header_str(&response, "last-modified").and_then(parse_http_date),
        }))
    }

    async fn delete(&self, key: &BlobKey) -> Result<()> {
        let path = self.path(Some(&self.object(key)));
        let response = self
            .send(Method::DELETE, &path, &[], Bytes::new(), &[])
            .await
            .map_err(|e| transport_error("delete", key.as_str(), e))?;
        if response.status().is_success() || response.status() == StatusCode::NOT_FOUND {
            Ok(())
        } else {
            Err(status_error("delete", key.as_str(), response).await)
        }
    }

    async fn delete_prefix(&self, prefix: &BlobKey) -> Result<()> {
        for meta in self.list(prefix).await? {
            self.delete(&meta.key).await?;
        }
        self.delete(prefix).await
    }

    async fn list(&self, prefix: &BlobKey) -> Result<Vec<BlobMeta>> {
        // The trailing `/` keeps `a` from matching `ab/...`.
        let object_prefix = format!("{}/", self.object(prefix));
        let mut out = Vec::new();
        let mut token: Option<String> = None;
        loop {
            let (items, next) = self
                .list_page(prefix, &object_prefix, token.as_deref())
                .await?;
            out.extend(items);
            match next {
                Some(next) => token = Some(next),
                None => break,
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_keys_like_s3() {
        assert_eq!(uri_encode("a b/c+d~e", true), "a%20b/c%2Bd~e");
        assert_eq!(uri_encode("a/b", false), "a%2Fb");
        assert_eq!(uri_encode("ü", true), "%C3%BC");
    }

    #[test]
    fn reads_list_pages() {
        let xml = "<ListBucketResult><IsTruncated>true</IsTruncated>\
            <Contents><Key>p/a&amp;b.txt</Key><Size>3</Size><ETag>&quot;x&quot;</ETag></Contents>\
            <Contents><Key>p/c</Key><Size>5</Size></Contents>\
            <NextContinuationToken>tok</NextContinuationToken></ListBucketResult>";
        let blocks = xml_blocks(xml, "Contents");
        assert_eq!(blocks.len(), 2);
        assert_eq!(xml_tag(blocks[0], "Key").unwrap(), "p/a&b.txt");
        assert_eq!(xml_tag(blocks[0], "ETag").unwrap(), "\"x\"");
        assert_eq!(xml_tag(xml, "NextContinuationToken").unwrap(), "tok");
    }

    #[test]
    fn credentials_debug_hides_credentials() {
        let creds = S3Credentials {
            access_key_id: "AKID".into(),
            secret_access_key: "super-secret".into(),
            session_token: Some("tok-secret".into()),
        };
        let shown = format!("{creds:?}");
        assert!(
            !shown.contains("AKID")
                && !shown.contains("super-secret")
                && !shown.contains("tok-secret")
        );
    }

    #[test]
    fn addressing_styles() {
        let creds = S3Credentials {
            access_key_id: "a".into(),
            secret_access_key: "s".into(),
            session_token: None,
        };
        let store = S3BlobStore::with_credentials(
            S3Config {
                bucket: "b".into(),
                region: "us-east-1".into(),
                endpoint: Some("http://minio:9000".into()),
                path_style: None,
                prefix: Some("/pre/".into()),
            },
            reqwest::Client::new(),
            creds.clone(),
        )
        .unwrap();
        assert_eq!(store.host, "minio:9000");
        let key = BlobKey::new("uploads/x y").unwrap();
        assert_eq!(
            store.path(Some(&store.object(&key))),
            "/b/pre/uploads/x%20y"
        );

        let aws = S3BlobStore::with_credentials(
            S3Config {
                bucket: "b".into(),
                region: "eu-west-1".into(),
                endpoint: None,
                path_style: None,
                prefix: None,
            },
            reqwest::Client::new(),
            creds,
        )
        .unwrap();
        assert_eq!(aws.host, "b.s3.eu-west-1.amazonaws.com");
        assert_eq!(aws.path(Some("k")), "/k");
    }

    /// Runs only when `VIZIER_TEST_S3_ENDPOINT` is set (MinIO in CI): creates a fresh bucket,
    /// runs the shared conformance cases, a multipart upload and the probe, then removes it.
    mod live {
        use super::*;

        fn env(name: &str, default: &str) -> String {
            std::env::var(name).unwrap_or_else(|_| default.to_string())
        }

        fn store_for(bucket: &str, endpoint: &str) -> S3BlobStore {
            S3BlobStore::with_credentials(
                S3Config {
                    bucket: bucket.to_string(),
                    region: env("VIZIER_TEST_S3_REGION", "us-east-1"),
                    endpoint: Some(endpoint.to_string()),
                    path_style: Some(true),
                    prefix: Some("vizier".into()),
                },
                reqwest::Client::new(),
                S3Credentials {
                    access_key_id: env("VIZIER_TEST_S3_ACCESS_KEY_ID", "vizier"),
                    secret_access_key: env(
                        "VIZIER_TEST_S3_SECRET_ACCESS_KEY",
                        "vizier-test-secret",
                    ),
                    session_token: None,
                },
            )
            .unwrap()
        }

        async fn bucket_request(store: &S3BlobStore, method: Method) {
            let response = store
                .send(method.clone(), &store.path(None), &[], Bytes::new(), &[])
                .await
                .unwrap();
            if !response.status().is_success() {
                panic!(
                    "{method} bucket: {}",
                    status_error("bucket", "", response).await
                );
            }
        }

        #[tokio::test]
        async fn conformance_against_a_live_bucket() {
            let Ok(endpoint) = std::env::var("VIZIER_TEST_S3_ENDPOINT") else {
                return;
            };
            let bucket = format!("vizier-test-{}", uuid::Uuid::new_v4());
            let mut store = store_for(&bucket, &endpoint);
            bucket_request(&store, Method::PUT).await;

            store.probe().await.unwrap();
            crate::storage::blob::conformance::run(&store).await;

            // Multipart with small parts (S3's minimum part size is 5 MiB).
            store.multipart_threshold = 6 * 1024 * 1024;
            store.part_size = 5 * 1024 * 1024;
            let big: Vec<u8> = (0..12 * 1024 * 1024u32).map(|i| (i % 253) as u8).collect();
            let key = BlobKey::new("multi/big.bin").unwrap();
            let meta = store.put(&key, Bytes::from(big.clone())).await.unwrap();
            assert_eq!(meta.size, big.len() as u64);
            assert_eq!(
                store.head(&key).await.unwrap().unwrap().size,
                big.len() as u64
            );
            let (_, stream) = store
                .get_range(&key, Some((5_000_000, 5_300_000)))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                crate::storage::blob::conformance::collect(stream).await,
                &big[5_000_000..=5_300_000]
            );
            let chunks: Vec<std::io::Result<Bytes>> = big
                .chunks(1024 * 1024)
                .map(|c| Ok(Bytes::copy_from_slice(c)))
                .collect();
            let key2 = BlobKey::new("multi/streamed.bin").unwrap();
            store
                .put_stream(
                    &key2,
                    Box::pin(futures::stream::iter(chunks)),
                    Some(big.len() as u64),
                )
                .await
                .unwrap();
            assert_eq!(store.get(&key2).await.unwrap().unwrap().len(), big.len());

            // Paging: more objects than one small page.
            for i in 0..5 {
                store
                    .put(
                        &BlobKey::new(&format!("page/{i}")).unwrap(),
                        Bytes::from_static(b"p"),
                    )
                    .await
                    .unwrap();
            }
            assert_eq!(
                store
                    .list(&BlobKey::new("page").unwrap())
                    .await
                    .unwrap()
                    .len(),
                5
            );

            // An error never shows the secret.
            let wrong = S3BlobStore::with_credentials(
                S3Config {
                    bucket: bucket.clone(),
                    region: "us-east-1".into(),
                    endpoint: Some(endpoint.clone()),
                    path_style: Some(true),
                    prefix: None,
                },
                reqwest::Client::new(),
                S3Credentials {
                    access_key_id: "vizier".into(),
                    secret_access_key: "definitely-wrong-secret".into(),
                    session_token: Some("token-value".into()),
                },
            )
            .unwrap();
            let err = wrong.probe().await.unwrap_err().to_string();
            assert!(!err.contains("definitely-wrong-secret") && !err.contains("token-value"));
            assert!(!err.contains("Signature="));

            store.prefix = None;
            for meta in store.list(&BlobKey::new("vizier").unwrap()).await.unwrap() {
                store.delete(&meta.key).await.unwrap();
            }
            bucket_request(&store, Method::DELETE).await;
        }
    }
}
