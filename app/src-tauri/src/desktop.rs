//! The desktop conveniences, as plain functions (contracts/ipc-protocol.md §4–§6): deep links,
//! notification navigation, native save/open, and the rule for which URLs stay in the window.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};

use crate::{engine::Engine, ipc};

/// `vizier://agents/dp-a/chat` -> `/agents/dp-a/chat`. Anything else is not ours.
pub fn deep_link_href(url: &str) -> Option<String> {
    let rest = url.strip_prefix("vizier://")?;
    let rest = rest.trim_start_matches('/');
    if rest.contains("://") || rest.starts_with('\\') {
        return None;
    }
    Some(format!("/{rest}"))
}

/// Which URLs may load in the main window; everything else opens in the default browser.
pub fn navigation_allowed(url: &str, dev_url: Option<&str>) -> bool {
    const APP_ORIGINS: [&str; 6] = [
        "tauri://localhost",
        "http://tauri.localhost",
        "https://tauri.localhost",
        "vizier-asset://localhost",
        "http://vizier-asset.localhost",
        "about:blank",
    ];
    let on_origin = |origin: &str| {
        url == origin
            || url.strip_prefix(origin).is_some_and(|rest| rest.starts_with('/') || rest.starts_with('?') || rest.starts_with('#'))
    };
    // `Url::to_string` ends the dev origin with a `/`; compare on the bare origin.
    let dev_url = dev_url.map(|d| d.trim_end_matches('/'));
    APP_ORIGINS.iter().any(|o| on_origin(o)) || dev_url.is_some_and(on_origin)
}

const NOTIFY_FOCUS_WINDOW: Duration = Duration::from_secs(60);

/// Where a `desktop://navigate` goes, with the two cases that need care: a link that arrives
/// before the WebUI is listening (cold start via `vizier://`), and a notification whose click
/// the platform does not report (research R11).
#[derive(Default)]
pub struct Navigator {
    inner: Mutex<NavState>,
}

#[derive(Default)]
struct NavState {
    listening: bool,
    pending: Option<String>,
    notified_at: Option<Instant>,
}

impl Navigator {
    /// `Some(href)` to emit now, `None` when it was kept until the WebUI is ready.
    pub fn request(&self, href: String) -> Option<String> {
        let mut s = self.inner.lock().unwrap();
        if s.listening {
            Some(href)
        } else {
            s.pending = Some(href);
            None
        }
    }

    /// The WebUI is listening; returns a link that arrived earlier.
    pub fn ready(&self) -> Option<String> {
        let mut s = self.inner.lock().unwrap();
        s.listening = true;
        s.pending.take()
    }

    pub fn note_notification(&self) {
        self.inner.lock().unwrap().notified_at = Some(Instant::now());
    }

    /// On window focus: within a minute of a notification, open the notifications panel.
    pub fn on_focus(&self) -> Option<String> {
        let mut s = self.inner.lock().unwrap();
        match s.notified_at.take() {
            Some(at) if at.elapsed() <= NOTIFY_FOCUS_WINDOW => Some("/notifications".to_string()),
            _ => None,
        }
    }

    /// A notification click the platform did report: no fallback is needed any more.
    pub fn clicked(&self) {
        self.inner.lock().unwrap().notified_at = None;
    }
}

/// A file name that is safe to join to a directory.
pub fn safe_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c })
        .collect();
    let cleaned = cleaned.trim_matches('.').trim().to_string();
    if cleaned.is_empty() { "file".to_string() } else { cleaned }
}

/// Streams a GET of `/api/v1{path}` to `dest` (the save dialog's choice).
pub async fn save_file(engine: &Engine, path: &str, dest: &Path) -> Result<(), String> {
    let reply = ipc::asset(engine, "GET", &format!("/api/v1{path}"), None)
        .await
        .map_err(|e| e.message)?;
    if !(200..300).contains(&reply.status) {
        return Err(format!("the file could not be read (status {})", reply.status));
    }
    tokio::fs::write(dest, reply.body).await.map_err(|e| e.to_string())
}

/// Saves to a temp file for the default app to open; returns where it went.
pub async fn open_file_path(engine: &Engine, path: &str, filename: &str) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("vizier");
    tokio::fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;
    let dest = dir.join(safe_filename(filename));
    save_file(engine, path, &dest).await?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_links() {
        assert_eq!(deep_link_href("vizier://agents/dp-a/chat").as_deref(), Some("/agents/dp-a/chat"));
        assert_eq!(deep_link_href("vizier:///x").as_deref(), Some("/x"));
        assert_eq!(deep_link_href("https://evil.example"), None);
        assert_eq!(deep_link_href("vizier://http://evil"), None);
    }

    #[test]
    fn navigation_guard() {
        assert!(navigation_allowed("tauri://localhost/agents", None));
        assert!(navigation_allowed("http://tauri.localhost/", None));
        assert!(!navigation_allowed("https://example.com", None));
        assert!(!navigation_allowed("tauri://localhost.evil.com/", None));
        assert!(navigation_allowed("http://localhost:5173/x", Some("http://localhost:5173")));
        assert!(!navigation_allowed("http://localhost:5173/x", None));
        // The configured dev URL arrives with a trailing slash.
        assert!(navigation_allowed("http://localhost:5173/agents/a", Some("http://localhost:5173/")));
        assert!(navigation_allowed("http://localhost:5173/", Some("http://localhost:5173/")));
    }

    #[test]
    fn links_wait_for_the_webui() {
        let nav = Navigator::default();
        assert_eq!(nav.request("/a".into()), None);
        assert_eq!(nav.ready().as_deref(), Some("/a"));
        assert_eq!(nav.request("/b".into()).as_deref(), Some("/b"));
    }

    #[test]
    fn focus_after_a_notification_opens_the_panel_once() {
        let nav = Navigator::default();
        assert_eq!(nav.on_focus(), None);
        nav.note_notification();
        assert_eq!(nav.on_focus().as_deref(), Some("/notifications"));
        assert_eq!(nav.on_focus(), None);
        nav.note_notification();
        nav.clicked();
        assert_eq!(nav.on_focus(), None);
    }

    #[test]
    fn filenames_cannot_escape() {
        assert_eq!(safe_filename("../../etc/passwd"), "_.._etc_passwd");
        assert_eq!(safe_filename(""), "file");
    }
}
