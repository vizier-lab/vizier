//! `bedrock_sso_session`: one encrypted session row per profile (spec 044, research D3,
//! FR-009a, SC-017a).
//!
//! The row is `(profile TEXT PRIMARY KEY, data TEXT NOT NULL)`, where `data` is the
//! `SecretCipher` envelope of an [`SsoSessionSecret`] built by
//! `encryption::fields::protect_sso_session`. The profile is the row key *and* part of the AAD,
//! so an envelope copied into another profile's row fails to decrypt instead of being replayed
//! as that connection.

use anyhow::{Context, Result};

use crate::storage::sql::{self, OptionalExtension, params};

use crate::{
    schema::SsoSessionSecret,
    storage::{
        sso_session::{SsoSessionStorage, SsoWrite},
        sqlite::SqlStorage,
    },
};

impl SqlStorage {
    /// The stored envelope for `profile`, without decrypting it.
    fn sso_session_envelope(&self, profile: &str) -> Result<Option<String>> {
        let conn = self.conn.lock()?;
        let data = conn
            .query_row(
                "SELECT data FROM bedrock_sso_session WHERE profile = ?1",
                params![profile],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(data)
    }
}

#[async_trait::async_trait]
impl SsoSessionStorage for SqlStorage {
    async fn get_sso_session(&self, profile: &str) -> Result<Option<SsoSessionSecret>> {
        let Some(envelope) = self.sso_session_envelope(profile)? else {
            return Ok(None);
        };
        let json = crate::encryption::fields::decrypt_sso_session(&self.cipher, profile, &envelope)
            .with_context(|| format!("bedrock sso session for profile {profile:?}"))?;
        let secret = serde_json::from_str::<SsoSessionSecret>(&json)
            .with_context(|| format!("bedrock sso session for profile {profile:?}"))?;
        Ok(Some(secret))
    }

    async fn put_sso_session(
        &self,
        profile: &str,
        secret: &SsoSessionSecret,
        expected_revision: Option<u64>,
    ) -> Result<SsoWrite> {
        let mut written = secret.clone();
        written.revision = expected_revision.unwrap_or(0) + 1;
        let envelope =
            crate::encryption::fields::protect_sso_session(&self.cipher, profile, &written.to_json())?;

        let conn = self.conn.lock()?;
        // `IMMEDIATE` takes the write lock *before* the read below, so a second process on the
        // same workspace queues here rather than reading the old revision alongside this one and
        // then failing on the upgrade with "database is locked". The loser waits, re-reads, sees
        // a newer revision and is told to re-read the winner — a clean `Conflict`, which is what
        // makes renewal idempotent across processes (FR-014, spec edge case "two Vizier
        // processes"). `busy_timeout` is what turns the wait into patience instead of an error.
        // `conn.transaction()` issues `BEGIN IMMEDIATE` on SQLite (storage/sql/mod.rs), so the
        // write lock is still taken before the read below.
        let tx = conn.transaction()?;
        let current = tx
            .query_row(
                "SELECT data FROM bedrock_sso_session WHERE profile = ?1",
                params![profile],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        let stored_revision = match (&current, expected_revision) {
            (None, None) => None,
            (Some(envelope), Some(_)) => {
                let json = crate::encryption::fields::decrypt_sso_session(
                    &self.cipher,
                    profile,
                    envelope,
                )
                .with_context(|| format!("bedrock sso session for profile {profile:?}"))?;
                let stored = serde_json::from_str::<SsoSessionSecret>(&json)
                    .with_context(|| format!("bedrock sso session for profile {profile:?}"))?;
                Some(stored.revision)
            }
            // The row appeared (a create lost the race) or vanished (an update lost it).
            _ => return Ok(SsoWrite::Conflict),
        };
        if stored_revision != expected_revision {
            return Ok(SsoWrite::Conflict);
        }

        tx.execute(
            "INSERT INTO bedrock_sso_session (profile, data) VALUES (?1, ?2)
             ON CONFLICT(profile) DO UPDATE SET data = excluded.data",
            params![profile, envelope],
        )?;
        tx.commit()?;
        Ok(SsoWrite::Written(written.revision))
    }

    async fn delete_sso_session(&self, profile: &str) -> Result<()> {
        let conn = self.conn.lock()?;
        conn.execute(
            "DELETE FROM bedrock_sso_session WHERE profile = ?1",
            params![profile],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::{Duration, Utc};
    use parking_lot::Mutex;

    use super::*;
    use crate::{
        encryption::{IntegrationSecretPaths, SecretCipher},
        storage::document::LocalDocumentStore,
    };

    /// An encrypted store over an in-memory database, so a test can see the envelope itself.
    fn setup() -> (SqliteStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        SqliteStorage::init_schema(&conn).unwrap();
        let storage = SqliteStorage::with_secrets(
            Arc::new(Mutex::new(conn)),
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
            SecretCipher::from_key_for_test([7u8; 32]),
            Arc::new(NoSecretPaths),
        );
        (storage, dir)
    }

    struct NoSecretPaths;

    impl IntegrationSecretPaths for NoSecretPaths {
        fn secret_paths(&self, _kind: &str) -> Option<Vec<String>> {
            None
        }
    }

    fn secret(access_token: &str) -> SsoSessionSecret {
        let now = Utc::now();
        SsoSessionSecret {
            access_token: access_token.into(),
            refresh_token: Some("refresh-token".into()),
            token_expires_at: now + Duration::hours(1),
            client_id: "client-id".into(),
            client_secret: "client-secret".into(),
            client_secret_expires_at: Some(now + Duration::days(90)),
            issued_at: now,
            revision: 0,
        }
    }

    fn raw_row(storage: &SqliteStorage, profile: &str) -> String {
        let conn = storage.conn.lock();
        conn.query_row(
            "SELECT data FROM bedrock_sso_session WHERE profile = ?1",
            rusqlite::params![profile],
            |row| row.get(0),
        )
        .unwrap()
    }

    /// Two storage handles over one file, the way two Vizier processes share a workspace: separate
    /// connections, one workspace key, one busy timeout each (FR-014, spec edge case "two Vizier
    /// processes").
    fn two_processes() -> (SqliteStorage, SqliteStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vizier.sqlite3");
        let open = || {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
                .unwrap();
            SqliteStorage::init_schema(&conn).unwrap();
            SqliteStorage::with_secrets(
                Arc::new(Mutex::new(conn)),
                Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
                SecretCipher::from_key_for_test([7u8; 32]),
                Arc::new(NoSecretPaths),
            )
        };
        (open(), open(), dir)
    }

    #[tokio::test]
    async fn a_written_session_reads_back_and_is_stored_as_an_envelope() {
        let (storage, _dir) = setup();
        let written = secret("access-token-one");
        assert_eq!(
            storage
                .put_sso_session("acme-bedrock", &written, None)
                .await
                .unwrap(),
            SsoWrite::Written(1)
        );

        let read = storage.get_sso_session("acme-bedrock").await.unwrap().unwrap();
        assert_eq!(read.access_token, "access-token-one");
        assert_eq!(read.refresh_token.as_deref(), Some("refresh-token"));
        assert_eq!(read.client_secret, "client-secret");
        // The store owns the revision: a create is revision 1 whatever the caller passed.
        assert_eq!(read.revision, 1);

        // Nothing readable on disk: an envelope, and no token in clear.
        let stored = raw_row(&storage, "acme-bedrock");
        assert!(stored.starts_with("enc:v1:"), "{stored}");
        for clear in ["access-token-one", "refresh-token", "client-secret"] {
            assert!(!stored.contains(clear), "{stored}");
        }
    }

    #[tokio::test]
    async fn another_profiles_envelope_does_not_decrypt() {
        let (storage, _dir) = setup();
        storage
            .put_sso_session("acme-bedrock", &secret("acme-token"), None)
            .await
            .unwrap();

        // Move the row's envelope to a second profile: the AAD names the profile, so the copy
        // must fail to open rather than hand one connection another's session (FR-009b).
        let envelope = raw_row(&storage, "acme-bedrock");
        {
            let conn = storage.conn.lock();
            conn.execute(
                "INSERT INTO bedrock_sso_session (profile, data) VALUES (?1, ?2)",
                rusqlite::params!["other-profile", envelope],
            )
            .unwrap();
        }

        let error = storage
            .get_sso_session("other-profile")
            .await
            .expect_err("a copied envelope must not decrypt");
        assert!(
            format!("{error:#}").contains("nothing was deleted"),
            "{error:#}"
        );

        // The original still reads, and nothing was deleted (SC-019).
        assert_eq!(
            storage
                .get_sso_session("acme-bedrock")
                .await
                .unwrap()
                .unwrap()
                .access_token,
            "acme-token"
        );
        assert_eq!(
            storage
                .get_sso_session("other-profile")
                .await
                .is_err(),
            true
        );
    }

    #[tokio::test]
    async fn the_compare_and_swap_refuses_a_stale_revision() {
        let (storage, _dir) = setup();
        assert_eq!(
            storage
                .put_sso_session("acme-bedrock", &secret("first"), None)
                .await
                .unwrap(),
            SsoWrite::Written(1)
        );

        // A second writer that read revision 1 wins, and the store says so.
        assert_eq!(
            storage
                .put_sso_session("acme-bedrock", &secret("second"), Some(1))
                .await
                .unwrap(),
            SsoWrite::Written(2)
        );
        // A writer still holding revision 1 writes nothing and changes nothing.
        assert_eq!(
            storage
                .put_sso_session("acme-bedrock", &secret("third"), Some(1))
                .await
                .unwrap(),
            SsoWrite::Conflict
        );
        // A create that lost the race also refuses.
        assert_eq!(
            storage
                .put_sso_session("acme-bedrock", &secret("fourth"), None)
                .await
                .unwrap(),
            SsoWrite::Conflict
        );

        let read = storage.get_sso_session("acme-bedrock").await.unwrap().unwrap();
        assert_eq!(read.access_token, "second");
        assert_eq!(read.revision, 2);
    }

    #[tokio::test]
    async fn signing_out_twice_is_not_an_error() {
        let (storage, _dir) = setup();
        storage
            .put_sso_session("acme-bedrock", &secret("token"), None)
            .await
            .unwrap();
        storage.delete_sso_session("acme-bedrock").await.unwrap();
        assert!(storage.get_sso_session("acme-bedrock").await.unwrap().is_none());
        // Already signed out: still fine (FR-009c).
        storage.delete_sso_session("acme-bedrock").await.unwrap();
    }

    /// T056: two processes on one workspace renew at once. Each read the same revision, one wins,
    /// and the loser is told to re-read instead of overwriting the winner — and its retry with the
    /// winner's revision writes (FR-014, spec edge case "two Vizier processes").
    #[tokio::test]
    async fn the_losing_writer_rereads_the_winner_and_its_retry_writes() {
        let (first, second, _dir) = two_processes();
        assert_eq!(
            first
                .put_sso_session("acme-bedrock", &secret("first"), None)
                .await
                .unwrap(),
            SsoWrite::Written(1)
        );

        // Both processes hold revision 1, as they would after both reading the same row.
        assert_eq!(
            first
                .get_sso_session("acme-bedrock")
                .await
                .unwrap()
                .unwrap()
                .revision,
            1
        );
        assert_eq!(
            second
                .get_sso_session("acme-bedrock")
                .await
                .unwrap()
                .unwrap()
                .revision,
            1
        );

        // The first process renews and wins.
        assert_eq!(
            first
                .put_sso_session("acme-bedrock", &secret("renewed-by-first"), Some(1))
                .await
                .unwrap(),
            SsoWrite::Written(2)
        );

        // The other process, still holding revision 1, writes nothing and is told to re-read.
        assert_eq!(
            second
                .put_sso_session("acme-bedrock", &secret("renewed-by-second"), Some(1))
                .await
                .unwrap(),
            SsoWrite::Conflict
        );
        let winner = second.get_sso_session("acme-bedrock").await.unwrap().unwrap();
        assert_eq!(winner.access_token, "renewed-by-first");
        assert_eq!(winner.revision, 2);

        // Its retry, with the winner's revision, writes: the store has one row, one step on.
        assert_eq!(
            second
                .put_sso_session("acme-bedrock", &secret("retried-by-second"), Some(2))
                .await
                .unwrap(),
            SsoWrite::Written(3)
        );
        let after = first.get_sso_session("acme-bedrock").await.unwrap().unwrap();
        assert_eq!(after.access_token, "retried-by-second");
        assert_eq!(after.revision, 3);
    }

    /// T056: a write racing across two processes waits on the lock instead of failing at it, so
    /// exactly one writer wins and the other gets `Conflict` — never "database is locked" (FR-014).
    #[test]
    fn a_racing_write_across_two_processes_waits_instead_of_failing() {
        let (first, second, _dir) = two_processes();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime
            .block_on(first.put_sso_session("acme-bedrock", &secret("first"), None))
            .unwrap();

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let outcomes = std::thread::scope(|scope| {
            let handles: Vec<_> = [(&first, "renewed-by-first"), (&second, "renewed-by-second")]
                .into_iter()
                .map(|(storage, token)| {
                    let barrier = std::sync::Arc::clone(&barrier);
                    scope.spawn(move || {
                        let runtime = tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()
                            .unwrap();
                        // Both processes hold revision 1 and write at the same instant.
                        barrier.wait();
                        runtime
                            .block_on(storage.put_sso_session(
                                "acme-bedrock",
                                &secret(token),
                                Some(1),
                            ))
                            .unwrap()
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect::<Vec<_>>()
        });

        assert_eq!(
            outcomes.iter().filter(|out| **out == SsoWrite::Written(2)).count(),
            1,
            "{outcomes:?}"
        );
        assert_eq!(
            outcomes.iter().filter(|out| **out == SsoWrite::Conflict).count(),
            1,
            "{outcomes:?}"
        );
        assert_eq!(
            runtime
                .block_on(first.get_sso_session("acme-bedrock"))
                .unwrap()
                .unwrap()
                .revision,
            2
        );
    }
}
