//! `Content-Length` framing and message classification for JSON-RPC over stdio.

use serde_json::Value;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{Result, error::VizierError};

/// Largest frame accepted from a server.
pub const MAX_FRAME: usize = 64 * 1024 * 1024;

/// What a message from the server is.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    /// An answer to one of our requests.
    Response { id: i64, result: std::result::Result<Value, RpcError> },
    /// The server asks us for something and expects an answer.
    Request { id: Value, method: String, params: Value },
    Notification { method: String, params: Value },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl From<RpcError> for VizierError {
    fn from(e: RpcError) -> Self {
        VizierError(format!("language server error {}: {}", e.code, e.message))
    }
}

/// Read one frame. `Ok(None)` is a clean end of stream.
pub async fn read_frame<R: AsyncBufRead + AsyncRead + Unpin>(reader: &mut R) -> Result<Option<Value>> {
    let mut length: Option<usize> = None;
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader
            .read_line(&mut line)
            .await
            .map_err(|e| VizierError(format!("language server read failed: {e}")))?;
        if n == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            if length.is_some() {
                break;
            }
            continue;
        }
        if let Some((name, value)) = trimmed.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| VizierError(format!("bad Content-Length: {}", value.trim())))?,
            );
        }
    }
    let length = length.ok_or_else(|| VizierError("frame without Content-Length".into()))?;
    if length > MAX_FRAME {
        return Err(VizierError(format!("frame of {length} bytes is over the limit")));
    }
    let mut body = vec![0u8; length];
    reader
        .read_exact(&mut body)
        .await
        .map_err(|e| VizierError(format!("language server read failed: {e}")))?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|e| VizierError(format!("language server sent invalid JSON: {e}")))
}

/// Write one frame.
pub async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, message: &Value) -> Result<()> {
    let body = serde_json::to_vec(message).map_err(|e| VizierError(format!("encode failed: {e}")))?;
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let io = |e: std::io::Error| VizierError(format!("language server write failed: {e}"));
    writer.write_all(header.as_bytes()).await.map_err(io)?;
    writer.write_all(&body).await.map_err(io)?;
    writer.flush().await.map_err(io)
}

/// Sort a decoded frame into response, server request or notification.
pub fn classify(message: Value) -> Option<Incoming> {
    let method = message.get("method").and_then(Value::as_str).map(str::to_owned);
    let id = message.get("id").cloned().filter(|v| !v.is_null());
    match (method, id) {
        (Some(method), Some(id)) => Some(Incoming::Request {
            id,
            method,
            params: message.get("params").cloned().unwrap_or(Value::Null),
        }),
        (Some(method), None) => Some(Incoming::Notification {
            method,
            params: message.get("params").cloned().unwrap_or(Value::Null),
        }),
        (None, Some(id)) => {
            let id = id.as_i64()?;
            if let Some(error) = message.get("error") {
                Some(Incoming::Response {
                    id,
                    result: Err(RpcError {
                        code: error.get("code").and_then(Value::as_i64).unwrap_or(0),
                        message: error
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_owned(),
                    }),
                })
            } else {
                Some(Incoming::Response {
                    id,
                    result: Ok(message.get("result").cloned().unwrap_or(Value::Null)),
                })
            }
        }
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::io::BufReader;

    async fn read_all(bytes: &[u8]) -> Vec<Value> {
        let mut reader = BufReader::new(bytes);
        let mut out = vec![];
        while let Some(v) = read_frame(&mut reader).await.unwrap() {
            out.push(v);
        }
        out
    }

    fn frame(v: &Value) -> Vec<u8> {
        let body = serde_json::to_vec(v).unwrap();
        let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        out.extend(body);
        out
    }

    #[tokio::test]
    async fn several_frames_in_one_read() {
        let mut bytes = frame(&json!({"id": 1}));
        bytes.extend(frame(&json!({"id": 2})));
        assert_eq!(read_all(&bytes).await, vec![json!({"id": 1}), json!({"id": 2})]);
    }

    #[tokio::test]
    async fn a_frame_split_across_reads() {
        let (mut tx, rx) = tokio::io::duplex(8);
        let bytes = frame(&json!({"method": "x", "params": {"long": "a".repeat(100)}}));
        let writer = tokio::spawn(async move {
            for chunk in bytes.chunks(5) {
                tx.write_all(chunk).await.unwrap();
                tokio::task::yield_now().await;
            }
        });
        let mut reader = BufReader::new(rx);
        let got = read_frame(&mut reader).await.unwrap().unwrap();
        assert_eq!(got["method"], "x");
        writer.await.unwrap();
    }

    #[tokio::test]
    async fn headers_are_case_insensitive_and_extra_headers_ignored() {
        let body = br#"{"a":1}"#;
        let mut bytes =
            format!("content-length: {}\r\nContent-Type: x\r\n\r\n", body.len()).into_bytes();
        bytes.extend_from_slice(body);
        assert_eq!(read_all(&bytes).await, vec![json!({"a": 1})]);
    }

    #[tokio::test]
    async fn a_bad_length_is_an_error() {
        let mut reader = BufReader::new(&b"Content-Length: nope\r\n\r\n{}"[..]);
        assert!(read_frame(&mut reader).await.is_err());
    }

    #[tokio::test]
    async fn an_oversized_frame_is_refused_before_reading() {
        let header = format!("Content-Length: {}\r\n\r\n", MAX_FRAME + 1);
        let mut reader = BufReader::new(header.as_bytes());
        let err = read_frame(&mut reader).await.unwrap_err();
        assert!(err.0.contains("over the limit"));
    }

    #[tokio::test]
    async fn write_then_read_round_trips() {
        let mut buf = vec![];
        write_frame(&mut buf, &json!({"id": 7, "result": null})).await.unwrap();
        assert_eq!(read_all(&buf).await, vec![json!({"id": 7, "result": null})]);
    }

    #[test]
    fn classification() {
        assert!(matches!(
            classify(json!({"id": 1, "result": 5})),
            Some(Incoming::Response { id: 1, result: Ok(_) })
        ));
        assert!(matches!(
            classify(json!({"id": 1, "error": {"code": -32801, "message": "m"}})),
            Some(Incoming::Response { result: Err(RpcError { code: -32801, .. }), .. })
        ));
        assert!(matches!(
            classify(json!({"id": 3, "method": "workspace/applyEdit"})),
            Some(Incoming::Request { .. })
        ));
        assert!(matches!(
            classify(json!({"method": "$/progress", "params": {}})),
            Some(Incoming::Notification { .. })
        ));
    }
}
