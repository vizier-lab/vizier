use std::{collections::HashMap, sync::Arc};

use reqwest::{Method, Response, header};
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::{VizierError, throw_vizier_error},
    file_manager::FileManager,
    storage::{VizierStorage, session_file::SessionFileStorage},
};

/// The largest response `save_as_session_file` keeps; a bigger one is refused, not cut.
pub const MAX_SAVED_BYTES: usize = 100 * 1024 * 1024;

/// The name a saved file gets when neither the call, the server nor the URL names it.
const FALLBACK_NAME: &str = "download";

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct HttpClientArgs {
    #[schemars(description = "URL to send the request to")]
    pub url: String,
    #[schemars(description = "HTTP method (GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS)")]
    pub method: String,
    #[schemars(description = "HTTP headers as key-value pairs")]
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[schemars(description = "Request body (optional, for POST/PUT/PATCH)")]
    pub body: Option<String>,
    #[schemars(
        description = "Save the response body, byte for byte, as a file of this conversation instead of returning it as text. Use it to download files (PDFs, images, archives). Only a 2xx response is saved; any other is returned as text. Up to 100 MiB."
    )]
    #[serde(default)]
    pub save_as_session_file: bool,
    #[schemars(
        description = "Name for the saved file, such as report.pdf (a name only, no folder). Read only with save_as_session_file. Defaults to the server's Content-Disposition name, then the URL's last path segment. A session file of the same name is replaced."
    )]
    pub filename: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct HttpClientOutput {
    #[schemars(description = "HTTP status code")]
    pub status: u16,
    #[schemars(
        description = "Response body. Empty when the response was saved as a session file."
    )]
    pub body: String,
    #[schemars(description = "Response headers as key-value pairs")]
    pub headers: HashMap<String, String>,
    #[schemars(
        description = "The session file the body was saved as, when save_as_session_file was set and the response was 2xx"
    )]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_file: Option<SavedSessionFile>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SavedSessionFile {
    pub filename: String,
    pub mime_type: String,
    pub size: u64,
}

pub struct HttpClient {
    pub storage: Arc<VizierStorage>,
    pub file_manager: FileManager,
}

#[async_trait::async_trait]
impl VizierTool for HttpClient {
    type Input = HttpClientArgs;
    type Output = HttpClientOutput;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Web)
    }

    fn name() -> String {
        "http_client".to_string()
    }

    fn description(&self) -> String {
        "Make HTTP requests to interact with REST APIs. Construct your own headers and choose the appropriate HTTP method. Supports GET, POST, PUT, DELETE, PATCH, HEAD, and OPTIONS. Set save_as_session_file to download the response as a file of this conversation (read it with read_document_file / read_image_file, send it with send_attachment).".to_string()
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let method = Method::from_bytes(args.method.to_uppercase().as_bytes())
            .map_err(|e| VizierError(format!("Invalid HTTP method: {}", e)))?;

        // Check a caller-given name before anything is sent, so a bad one costs no request.
        let given_name = match (&args.filename, args.save_as_session_file) {
            (Some(name), true) => Some(checked_name(name)?),
            _ => None,
        };

        let client = reqwest::Client::new();
        let mut request = client.request(method, &args.url);

        for (key, value) in &args.headers {
            request = request.header(key, value);
        }

        if let Some(body) = args.body {
            request = request.body(body);
        }

        let mut response = match request.send().await {
            Ok(response) => response,
            Err(err) => return throw_vizier_error("http_client: request error", err),
        };

        let status = response.status();
        let headers: HashMap<String, String> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        if args.save_as_session_file && status.is_success() {
            let content_type = response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);
            let filename = match given_name {
                Some(name) => name,
                None => default_name(&response, &args.url, content_type.as_deref()),
            };
            let bytes = read_capped(&mut response, MAX_SAVED_BYTES).await?;
            let record = self
                .file_manager
                .upload(&filename, bytes)
                .await
                .map_err(|e| VizierError(e.to_string()))?;
            // The name decides the type when it can; otherwise trust the server.
            let mime_type = match (record.mime_type.as_str(), content_type) {
                ("application/octet-stream", Some(served)) => mime_only(&served),
                (guessed, _) => guessed.to_string(),
            };
            self.storage
                .save_session_file(
                    &ctx.session,
                    &record.filename,
                    &mime_type,
                    record.size,
                    &record.id,
                )
                .await
                .map_err(|e| VizierError(e.to_string()))?;
            return Ok(HttpClientOutput {
                status: status.as_u16(),
                body: String::new(),
                headers,
                session_file: Some(SavedSessionFile {
                    filename: record.filename,
                    mime_type,
                    size: record.size,
                }),
            });
        }

        let body = match response.text().await {
            Ok(body) => body,
            Err(err) => return throw_vizier_error("http_client: text error", err),
        };

        Ok(HttpClientOutput {
            status: status.as_u16(),
            body,
            headers,
            session_file: None,
        })
    }
}

/// Reads the whole body, refusing (rather than cutting) one over `limit`.
async fn read_capped(response: &mut Response, limit: usize) -> Result<Vec<u8>, VizierError> {
    if response
        .content_length()
        .is_some_and(|len| len > limit as u64)
    {
        return Err(too_large(limit));
    }
    let mut bytes = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if bytes.len() + chunk.len() > limit {
                    return Err(too_large(limit));
                }
                bytes.extend_from_slice(&chunk);
            }
            Ok(None) => return Ok(bytes),
            Err(err) => return throw_vizier_error("http_client: body error", err),
        }
    }
}

fn too_large(limit: usize) -> VizierError {
    VizierError(format!(
        "http_client: the response is larger than {} MiB, so it was not saved",
        limit / (1024 * 1024)
    ))
}

/// A caller-given name: a name only, no folder.
fn checked_name(name: &str) -> Result<String, VizierError> {
    let name = name.trim();
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\']) {
        return Err(VizierError(format!(
            "'{name}' is not a usable filename: give a name only, such as report.pdf"
        )));
    }
    Ok(name.to_string())
}

/// The server's Content-Disposition name, then the URL's last path segment, then
/// [`FALLBACK_NAME`]; given an extension from the content type when it has none.
fn default_name(response: &Response, url: &str, content_type: Option<&str>) -> String {
    let disposition = response
        .headers()
        .get(header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .and_then(disposition_name);
    let from_url = || {
        reqwest::Url::parse(url).ok().and_then(|url| {
            url.path_segments()
                .and_then(|mut segments| segments.next_back().map(str::to_string))
                .map(|segment| percent_decode(&segment))
        })
    };
    let name = disposition
        .or_else(from_url)
        .map(|name| sanitise(&name))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| FALLBACK_NAME.to_string());
    with_extension(name, content_type)
}

/// `filename*=UTF-8''…` first (RFC 6266), then `filename=`.
fn disposition_name(value: &str) -> Option<String> {
    let params: Vec<(String, String)> = value
        .split(';')
        .skip(1)
        .filter_map(|part| {
            let (key, value) = part.split_once('=')?;
            Some((key.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect();
    let extended = params
        .iter()
        .find(|(k, _)| k == "filename*")
        .and_then(|(_, v)| {
            let (_, encoded) = v.split_once("''")?;
            Some(percent_decode(encoded))
        });
    extended.or_else(|| {
        params
            .iter()
            .find(|(k, _)| k == "filename")
            .map(|(_, v)| v.trim_matches('"').to_string())
    })
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = |b: u8| (b as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Keeps the last path component and swaps characters no platform can hold for `_`.
fn sanitise(name: &str) -> String {
    let last = name.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = last
        .chars()
        .map(|c| match c {
            ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();
    cleaned.to_string()
}

fn with_extension(name: String, content_type: Option<&str>) -> String {
    if std::path::Path::new(&name).extension().is_some() {
        return name;
    }
    let extension = content_type.map(mime_only).and_then(|mime| {
        mime_guess::get_mime_extensions_str(&mime).and_then(|exts| exts.first().copied())
    });
    match extension {
        Some(ext) => format!("{name}.{ext}"),
        None => name,
    }
}

/// `text/html; charset=utf-8` → `text/html`.
fn mime_only(content_type: &str) -> String {
    content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_disposition_name_prefers_the_extended_form() {
        assert_eq!(
            disposition_name(
                r#"attachment; filename="a b.pdf"; filename*=UTF-8''r%C3%A9sum%C3%A9.pdf"#
            ),
            Some("résumé.pdf".into())
        );
        assert_eq!(
            disposition_name(r#"attachment; filename="report.pdf""#),
            Some("report.pdf".into())
        );
        assert_eq!(disposition_name("inline"), None);
    }

    #[test]
    fn a_name_is_cleaned_to_one_safe_component() {
        assert_eq!(sanitise("../../etc/passwd"), "passwd");
        assert_eq!(sanitise(r"C:\x\a:b?.txt"), "a_b_.txt");
        assert_eq!(sanitise(".."), "");
    }

    #[test]
    fn a_name_without_an_extension_gets_one_from_the_content_type() {
        assert_eq!(
            with_extension("file".into(), Some("application/pdf")),
            "file.pdf"
        );
        assert_eq!(
            with_extension("file.bin".into(), Some("application/pdf")),
            "file.bin"
        );
        assert_eq!(with_extension("file".into(), None), "file");
    }

    #[test]
    fn a_given_name_must_be_a_name_only() {
        assert!(checked_name("report.pdf").is_ok());
        assert!(checked_name("a/b.pdf").is_err());
        assert!(checked_name(" ").is_err());
        assert!(checked_name("..").is_err());
    }

    #[test]
    fn percent_decoding_leaves_bad_escapes_alone() {
        assert_eq!(percent_decode("a%20b"), "a b");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
    }
}
