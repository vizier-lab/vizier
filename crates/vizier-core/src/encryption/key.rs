//! Key resolution and validation (`contracts/encryption.md` §2, §5): the operator's environment
//! value, an explicit key file, or the generated `{workspace}/.runtime/encryption.key`.
//!
//! Every path fails closed. A missing explicit source is an error, never a reason to generate;
//! a generated key is never written over an existing file or over protected data; and a wrong
//! key is detected before the first write.

use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use ring::rand::{SecureRandom, SystemRandom};
use rusqlite::{Connection, OptionalExtension};
use zeroize::Zeroizing;

use crate::encryption::cipher::{SecretCipher, envelope_key_id};

/// The permanent state row holding the id of the key that protects this workspace.
pub const KEY_ID_STATE_KEY: &str = "secrets.key_id";
const KEY_FILE_HEADER: &str = "vizier-key-v1";
const KEY_LEN: usize = 32;

/// Where the key came from, for diagnostics. Never contains key material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeySource {
    Env,
    File(PathBuf),
    Generated(PathBuf),
    Test,
    Disabled,
}

impl KeySource {
    pub fn describe(&self) -> String {
        match self {
            Self::Env => "VIZIER_ENCRYPTION_KEY".into(),
            Self::File(path) => path.display().to_string(),
            Self::Generated(path) => path.display().to_string(),
            Self::Test => "test key".into(),
            Self::Disabled => "no encryption".into(),
        }
    }
}

/// The two environment sources, injected so tests never race on process environment variables.
#[derive(Debug, Clone, Default)]
pub struct KeyRequest {
    /// `VIZIER_ENCRYPTION_KEY`; `Some("")` is malformed, not unset.
    pub env_key: Option<String>,
    /// `VIZIER_ENCRYPTION_KEY_FILE`; set-but-missing is an error.
    pub env_file: Option<String>,
}

impl KeyRequest {
    pub fn from_env() -> Self {
        Self {
            env_key: std::env::var("VIZIER_ENCRYPTION_KEY").ok(),
            env_file: std::env::var("VIZIER_ENCRYPTION_KEY_FILE").ok(),
        }
    }
}

/// The default generated key location, next to `vizier.db`.
pub fn default_key_path(workspace: &str) -> PathBuf {
    Path::new(workspace).join(".runtime").join("encryption.key")
}

/// Resolve, validate and (only when nothing is protected) generate the workspace key.
pub fn load_or_generate(
    workspace: &str,
    conn: &Connection,
    request: &KeyRequest,
) -> Result<SecretCipher> {
    let (key, source) = resolve(workspace, conn, request)?;
    let key = Zeroizing::new(key);
    let cipher = SecretCipher::from_key(*key, source)?;
    validate(conn, cipher.key_id())?;
    write_key_id(conn, cipher.key_id())?;
    tracing::info!(source = cipher.source().describe(), "encryption key loaded");
    Ok(cipher)
}

fn resolve(
    workspace: &str,
    conn: &Connection,
    request: &KeyRequest,
) -> Result<([u8; KEY_LEN], KeySource)> {
    if let Some(value) = &request.env_key {
        return Ok((decode_key(value)?, KeySource::Env));
    }

    if let Some(path) = &request.env_file {
        let path = PathBuf::from(path);
        return Ok((read_key_file(&path)?, KeySource::File(path)));
    }

    let path = default_key_path(workspace);
    if path.exists() {
        return Ok((read_key_file(&path)?, KeySource::Generated(path)));
    }

    // Nothing was supplied and no default file exists. Generate only when there is no protected
    // data and no key identity yet; otherwise this is a missing key, not a fresh install.
    if let Some(_key_id) = scan_for_key_id(conn)? {
        bail!(
            "encrypted data exists but no encryption key was found; set VIZIER_ENCRYPTION_KEY or restore the key file at {}",
            path.display()
        );
    }

    let key = generate_key()?;
    write_key_file(&path, &key)?;
    Ok((key, KeySource::Generated(path)))
}

fn validate(conn: &Connection, key_id: &str) -> Result<()> {
    if let Some(stored) = read_key_id(conn)? {
        if stored != key_id {
            bail!("encryption key does not match this data");
        }
        return Ok(());
    }

    // A restored database may have lost its state row; the envelopes themselves still carry the
    // key id, so the first one is enough to detect a mismatch.
    if let Some(found) = scan_for_key_id(conn)?
        && found != key_id
    {
        bail!("encryption key does not match this data");
    }
    Ok(())
}

/// Decode the operator-supplied base64 key, or fail with the contract's malformed message.
pub fn decode_key(text: &str) -> Result<[u8; KEY_LEN]> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(malformed());
    }
    let decoded = Zeroizing::new(STANDARD.decode(trimmed).map_err(|_| malformed())?);
    if decoded.len() != KEY_LEN {
        return Err(malformed());
    }
    let mut key = [0u8; KEY_LEN];
    key.copy_from_slice(&decoded);
    Ok(key)
}

fn malformed() -> anyhow::Error {
    anyhow!("encryption key is malformed: expected 32 bytes encoded as base64")
}

fn generate_key() -> Result<[u8; KEY_LEN]> {
    let mut key = [0u8; KEY_LEN];
    SystemRandom::new()
        .fill(&mut key)
        .map_err(|_| anyhow!("could not read random bytes to generate the encryption key"))?;
    Ok(key)
}

fn read_key_file(path: &Path) -> Result<[u8; KEY_LEN]> {
    let text = Zeroizing::new(
        std::fs::read_to_string(path)
            .map_err(|e| anyhow!("encryption key is unreadable: {}: {e}", path.display()))?,
    );

    #[cfg(unix)]
    tighten_permissions(path);

    parse_key_text(&text)
}

/// Parse the two-line key file format: `vizier-key-v1` then base64 of exactly 32 bytes.
/// Extra blank lines are tolerated; any other extra line is malformed.
pub fn parse_key_text(text: &str) -> Result<[u8; KEY_LEN]> {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if lines.len() != 2 || lines[0] != KEY_FILE_HEADER {
        return Err(malformed());
    }
    decode_key(lines[1])
}

fn write_key_file(path: &Path, key: &[u8; KEY_LEN]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| write_error(path, &e))?;
    }

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options.open(path).map_err(|e| write_error(path, &e))?;
    let encoded = Zeroizing::new(STANDARD.encode(key));
    let body = Zeroizing::new(format!("{KEY_FILE_HEADER}\n{}\n", encoded.as_str()));
    file.write_all(body.as_bytes())
        .map_err(|e| write_error(path, &e))?;
    file.flush().map_err(|e| write_error(path, &e))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| write_error(path, &e))?;
    }
    #[cfg(not(unix))]
    tracing::warn!(
        path = %path.display(),
        "this platform cannot restrict the encryption key file to its owner; protect the workspace directory instead"
    );

    tracing::info!(path = %path.display(), "generated a new encryption key");
    Ok(())
}

fn write_error(path: &Path, error: &std::io::Error) -> anyhow::Error {
    anyhow!(
        "could not write the encryption key file at {}: {error}",
        path.display()
    )
}

#[cfg(unix)]
fn tighten_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let Ok(metadata) = std::fs::metadata(path) else {
        return;
    };
    let mode = metadata.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        tracing::warn!(
            path = %path.display(),
            mode = format!("{mode:o}"),
            "the encryption key file is readable by other accounts; tightening it to 0600"
        );
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
}

pub fn read_key_id(conn: &Connection) -> Result<Option<String>> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM state WHERE key = ?1",
            rusqlite::params![KEY_ID_STATE_KEY],
            |row| row.get(0),
        )
        .optional()?;
    let Some(value) = value else {
        return Ok(None);
    };
    let parsed: serde_json::Value = serde_json::from_str(&value)?;
    Ok(parsed.as_str().map(str::to_string))
}

fn write_key_id(conn: &Connection, key_id: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO state (key, value) VALUES (?1, ?2)",
        rusqlite::params![KEY_ID_STATE_KEY, serde_json::to_string(key_id)?],
    )?;
    Ok(())
}

/// The first key id found in a protected column, used only when `state:secrets.key_id` is
/// absent. A missing table (unit tests, a fresh workspace) is simply nothing to find.
fn scan_for_key_id(conn: &Connection) -> Result<Option<String>> {
    if let Some(key_id) = read_key_id(conn)? {
        return Ok(Some(key_id));
    }
    for (table, column) in PROTECTED_SCAN {
        let sql = format!("SELECT {column} FROM {table}");
        let Ok(mut stmt) = conn.prepare(&sql) else {
            continue;
        };
        let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) else {
            continue;
        };
        for value in rows.flatten() {
            if let Some(index) = value.find(crate::encryption::cipher::ENVELOPE_PREFIX)
                && let Some(id) = envelope_key_id(&value[index..])
            {
                return Ok(Some(id.to_string()));
            }
        }
    }
    Ok(None)
}

const PROTECTED_SCAN: &[(&str, &str)] = &[
    ("agent_secret", "value"),
    ("provider_config", "data"),
    ("custom_provider", "data"),
    ("integration", "settings"),
    ("agent_config", "data"),
    ("global_config", "data"),
];

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 32] = [3u8; 32];

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE state (key TEXT PRIMARY KEY, value TEXT NOT NULL);")
            .unwrap();
        conn
    }

    fn workspace() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn workspace_str(dir: &tempfile::TempDir) -> String {
        dir.path().to_string_lossy().to_string()
    }

    fn request(key: Option<&[u8; 32]>, file: Option<&Path>) -> KeyRequest {
        KeyRequest {
            env_key: key.map(|k| STANDARD.encode(k)),
            env_file: file.map(|p| p.display().to_string()),
        }
    }

    fn write_file(path: &Path, key: &[u8; 32]) {
        std::fs::write(
            path,
            format!("{KEY_FILE_HEADER}\n{}\n", STANDARD.encode(key)),
        )
        .unwrap();
    }

    #[test]
    fn a_fresh_workspace_generates_a_key_file_and_state_id() {
        let conn = conn();
        let dir = workspace();
        let cipher = load_or_generate(&workspace_str(&dir), &conn, &KeyRequest::default()).unwrap();
        assert!(cipher.is_enabled());
        assert_eq!(
            cipher.source(),
            KeySource::Generated(default_key_path(&workspace_str(&dir)))
        );
        assert_eq!(
            read_key_id(&conn).unwrap().as_deref(),
            Some(cipher.key_id())
        );

        let text = std::fs::read_to_string(default_key_path(&workspace_str(&dir))).unwrap();
        assert_eq!(
            crate::encryption::cipher::key_id_for(&parse_key_text(&text).unwrap()),
            cipher.key_id()
        );
    }

    #[test]
    fn an_existing_key_file_is_loaded_not_replaced() {
        let conn = conn();
        let dir = workspace();
        let path = default_key_path(&workspace_str(&dir));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        write_file(&path, &KEY);

        let first = load_or_generate(&workspace_str(&dir), &conn, &KeyRequest::default()).unwrap();
        assert_eq!(first.key_id(), crate::encryption::cipher::key_id_for(&KEY));
        let before = std::fs::read(&path).unwrap();
        let second = load_or_generate(&workspace_str(&dir), &conn, &KeyRequest::default()).unwrap();
        assert_eq!(second.key_id(), first.key_id());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn a_supplied_env_key_wins_over_a_key_file() {
        let conn = conn();
        let dir = workspace();
        let default = default_key_path(&workspace_str(&dir));
        std::fs::create_dir_all(default.parent().unwrap()).unwrap();
        write_file(&default, &KEY);
        let explicit = dir.path().join("explicit.key");
        let other = [4u8; 32];
        write_file(&explicit, &other);

        let cipher = load_or_generate(
            &workspace_str(&dir),
            &conn,
            &request(Some(&[9u8; 32]), Some(&explicit)),
        )
        .unwrap();
        assert_eq!(cipher.source(), KeySource::Env);
        assert_eq!(
            cipher.key_id(),
            crate::encryption::cipher::key_id_for(&[9u8; 32])
        );
    }

    #[test]
    fn an_explicit_key_file_wins_over_the_default_one() {
        let conn = conn();
        let dir = workspace();
        let default = default_key_path(&workspace_str(&dir));
        std::fs::create_dir_all(default.parent().unwrap()).unwrap();
        write_file(&default, &KEY);
        let explicit = dir.path().join("explicit.key");
        let other = [4u8; 32];
        write_file(&explicit, &other);

        let cipher =
            load_or_generate(&workspace_str(&dir), &conn, &request(None, Some(&explicit))).unwrap();
        assert_eq!(cipher.source(), KeySource::File(explicit));
        assert_eq!(
            cipher.key_id(),
            crate::encryption::cipher::key_id_for(&other)
        );
    }

    #[test]
    fn a_missing_explicit_key_file_never_generates_one() {
        let conn = conn();
        let dir = workspace();
        let missing = dir.path().join("missing.key");
        let err = load_or_generate(&workspace_str(&dir), &conn, &request(None, Some(&missing)))
            .unwrap_err();
        assert!(format!("{err}").contains("unreadable"), "{err}");
        assert!(!missing.exists());
    }

    #[test]
    fn an_empty_env_value_is_malformed_not_unset() {
        let conn = conn();
        let dir = workspace();
        let err = load_or_generate(
            &workspace_str(&dir),
            &conn,
            &KeyRequest {
                env_key: Some(String::new()),
                env_file: None,
            },
        )
        .unwrap_err();
        assert!(format!("{err}").contains("malformed"), "{err}");
        assert!(!default_key_path(&workspace_str(&dir)).exists());
    }

    #[test]
    fn a_malformed_key_is_refused() {
        for value in ["not base64!!!", "AAAA", &STANDARD.encode([1u8; 31])] {
            let err = load_or_generate(
                "/tmp/irrelevant",
                &conn(),
                &KeyRequest {
                    env_key: Some(value.to_string()),
                    env_file: None,
                },
            )
            .unwrap_err();
            assert!(format!("{err}").contains("malformed"), "{err}");
        }
    }

    #[test]
    fn a_wrong_key_is_detected_before_any_write() {
        let conn = conn();
        let dir = workspace();
        let first = load_or_generate(&workspace_str(&dir), &conn, &KeyRequest::default()).unwrap();
        let state_before: String = conn
            .query_row(
                "SELECT value FROM state WHERE key = 'secrets.key_id'",
                [],
                |r| r.get(0),
            )
            .unwrap();

        let err = load_or_generate("/tmp/irrelevant", &conn, &request(Some(&[1u8; 32]), None))
            .unwrap_err();
        assert!(format!("{err}").contains("does not match"), "{err}");
        let state_after: String = conn
            .query_row(
                "SELECT value FROM state WHERE key = 'secrets.key_id'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(state_before, state_after);
        assert_eq!(first.key_id(), read_key_id(&conn).unwrap().unwrap());
    }

    #[test]
    fn a_missing_key_with_protected_data_aborts_and_generates_nothing() {
        let conn = conn();
        let dir = workspace();
        let cipher = load_or_generate(&workspace_str(&dir), &conn, &KeyRequest::default()).unwrap();
        let path = default_key_path(&workspace_str(&dir));
        std::fs::remove_file(&path).unwrap();
        conn.execute("DELETE FROM state", []).unwrap();
        // A protected row without a state key exercises the scan fallback.
        conn.execute_batch(
            "CREATE TABLE agent_secret (agent_id TEXT, name TEXT, value TEXT);
             INSERT INTO agent_secret VALUES ('dp-a', 'TOKEN', 'plain placeholder');",
        )
        .unwrap();
        let envelope = cipher.encrypt("x", "aad").unwrap();
        conn.execute(
            "UPDATE agent_secret SET value = ?1",
            rusqlite::params![envelope],
        )
        .unwrap();

        let err =
            load_or_generate(&workspace_str(&dir), &conn, &KeyRequest::default()).unwrap_err();
        assert!(
            format!("{err}").contains("no encryption key was found"),
            "{err}"
        );
        assert!(!path.exists());
    }

    #[test]
    fn restoring_the_correct_key_succeeds() {
        let conn = conn();
        let dir = workspace();
        let cipher = load_or_generate(&workspace_str(&dir), &conn, &KeyRequest::default()).unwrap();
        let path = default_key_path(&workspace_str(&dir));
        let backup = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();

        let err =
            load_or_generate(&workspace_str(&dir), &conn, &KeyRequest::default()).unwrap_err();
        assert!(
            format!("{err}").contains("no encryption key was found"),
            "{err}"
        );

        std::fs::write(&path, backup).unwrap();
        let again = load_or_generate(&workspace_str(&dir), &conn, &KeyRequest::default()).unwrap();
        assert_eq!(again.key_id(), cipher.key_id());
    }

    #[test]
    fn the_state_key_is_written_once_and_reused() {
        let conn = conn();
        let dir = workspace();
        let first = load_or_generate(&workspace_str(&dir), &conn, &KeyRequest::default()).unwrap();
        let stored = read_key_id(&conn).unwrap().unwrap();
        let second = load_or_generate(&workspace_str(&dir), &conn, &KeyRequest::default()).unwrap();
        assert_eq!(stored, second.key_id());
        assert_eq!(first.key_id(), second.key_id());
    }

    #[cfg(unix)]
    #[test]
    fn a_generated_key_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let conn = conn();
        let dir = workspace();
        load_or_generate(&workspace_str(&dir), &conn, &KeyRequest::default()).unwrap();
        let mode = std::fs::metadata(default_key_path(&workspace_str(&dir)))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn a_permissive_key_file_is_tightened_on_read() {
        use std::os::unix::fs::PermissionsExt;
        let conn = conn();
        let dir = workspace();
        let path = default_key_path(&workspace_str(&dir));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        write_file(&path, &KEY);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        load_or_generate(&workspace_str(&dir), &conn, &KeyRequest::default()).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn an_unwritable_workspace_reports_a_clear_error() {
        let conn = conn();
        let dir = workspace();
        // A regular file where `.runtime` must be a directory forces the write to fail.
        std::fs::write(dir.path().join(".runtime"), b"not a directory").unwrap();
        let err =
            load_or_generate(&workspace_str(&dir), &conn, &KeyRequest::default()).unwrap_err();
        assert!(
            format!("{err}").contains("could not write the encryption key file"),
            "{err}"
        );
    }

    #[test]
    fn an_unknown_key_file_version_is_malformed() {
        assert!(parse_key_text("vizier-key-v2\nAAAA\n").is_err());
        assert!(parse_key_text("vizier-key-v1\nAAAA\nextra\n").is_err());
        assert!(parse_key_text("vizier-key-v1\n").is_err());
    }

    #[test]
    fn a_key_file_tolerates_crlf_and_a_trailing_newline() {
        let text = format!("{KEY_FILE_HEADER}\r\n{}\r\n\r\n", STANDARD.encode(KEY));
        assert_eq!(parse_key_text(&text).unwrap(), KEY);
    }

    /// A restored connection may not have every application table yet; scanning for protected
    /// data tolerates missing tables, and only the `state` row the key id lives in is required.
    #[test]
    fn a_connection_with_only_the_state_table_can_still_load_a_key() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE state (key TEXT PRIMARY KEY, value TEXT NOT NULL);")
            .unwrap();
        let dir = workspace();
        let request = KeyRequest {
            env_key: Some(STANDARD.encode(KEY)),
            env_file: None,
        };
        let cipher = load_or_generate(&workspace_str(&dir), &conn, &request).unwrap();
        assert_eq!(cipher.key_id(), crate::encryption::cipher::key_id_for(&KEY));
    }
}
