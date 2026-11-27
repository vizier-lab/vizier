//! `platform_link`, `platform_connect_code` and `platform_link_event` (spec 069, data-model.md).
//!
//! A code is stored only as a hash. Redeeming deletes its row in the transaction that writes the
//! link, which is what makes a code work once.

use anyhow::{Context, Result};
use chrono::{DateTime, Local};

use crate::storage::sql::{self, OptionalExtension, params};

use crate::{
    platform_links::PlatformPerson,
    schema::ChannelPlatform,
    storage::{
        platform_link::{
            LinkAction, PlatformLink, PlatformLinkEvent, PlatformLinkStorage, RedeemOutcome,
        },
        sqlite::SqlStorage,
        user::UserProfile,
    },
    time::Timestamp,
};

pub(crate) fn init_platform_link_schema(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS platform_link (
            platform              TEXT NOT NULL,
            platform_user_id      TEXT NOT NULL,
            user_id               TEXT NOT NULL REFERENCES "user"(user_id) ON DELETE CASCADE,
            platform_username     TEXT,
            platform_display_name TEXT,
            linked_at             TEXT NOT NULL,
            PRIMARY KEY (platform, platform_user_id),
            UNIQUE (user_id, platform)
        );

        CREATE TABLE IF NOT EXISTS platform_connect_code (
            platform              TEXT NOT NULL,
            platform_user_id      TEXT NOT NULL,
            code_hash             TEXT NOT NULL UNIQUE,
            platform_username     TEXT,
            platform_display_name TEXT,
            created_at_ms         INTEGER NOT NULL,
            expires_at_ms         INTEGER NOT NULL,
            PRIMARY KEY (platform, platform_user_id)
        );

        CREATE TABLE IF NOT EXISTS platform_link_event (
            id               TEXT PRIMARY KEY,
            user_id          TEXT NOT NULL REFERENCES "user"(user_id) ON DELETE CASCADE,
            platform         TEXT NOT NULL,
            platform_user_id TEXT NOT NULL,
            action           TEXT NOT NULL,
            at               TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_platform_link_event_user
            ON platform_link_event(user_id, at);
        "#,
    )?;
    Ok(())
}

const LINK_COLUMNS: &str = r#"l.platform, l.platform_user_id, l.user_id, u.username,
    l.platform_username, l.platform_display_name, l.linked_at"#;

const LINK_FROM: &str = r#"platform_link l JOIN "user" u ON u.user_id = l.user_id"#;

/// A link as its columns come back, before the platform and time are parsed.
type LinkRow = (String, String, String, String, Option<String>, Option<String>, String);

fn read_link(row: &sql::Row) -> sql::Result<LinkRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
    ))
}

fn parse_platform(text: &str) -> Result<ChannelPlatform> {
    match text {
        "discord" => Ok(ChannelPlatform::Discord),
        "telegram" => Ok(ChannelPlatform::Telegram),
        other => Err(anyhow::anyhow!("unknown platform {other:?}")),
    }
}

fn parse_time(text: &str) -> Result<Timestamp> {
    Ok(DateTime::parse_from_rfc3339(text)
        .with_context(|| format!("not a stored time: {text:?}"))?
        .with_timezone(&Local))
}

fn link_from(row: LinkRow) -> Result<PlatformLink> {
    let (platform, platform_user_id, user_id, username, platform_username, display, at) = row;
    Ok(PlatformLink {
        platform: parse_platform(&platform)?,
        platform_user_id,
        user_id,
        username,
        platform_username,
        platform_display_name: display,
        linked_at: parse_time(&at)?,
    })
}

fn query_link(
    conn: &sql::Connection,
    condition: &str,
    first: &str,
    second: &str,
) -> Result<Option<PlatformLink>> {
    conn.query_row(
        &format!("SELECT {LINK_COLUMNS} FROM {LINK_FROM} WHERE {condition}"),
        params![first, second],
        read_link,
    )
    .optional()?
    .map(link_from)
    .transpose()
}

fn person_for_code(
    conn: &sql::Connection,
    code_hash: &str,
    now_ms: i64,
) -> Result<Option<PlatformPerson>> {
    conn.query_row(
        "SELECT platform, platform_user_id, platform_username, platform_display_name \
         FROM platform_connect_code WHERE code_hash = ?1 AND expires_at_ms > ?2",
        params![code_hash, now_ms],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        },
    )
    .optional()?
    .map(|(platform, id, username, display_name)| {
        Ok(PlatformPerson {
            platform: parse_platform(&platform)?,
            id,
            username,
            display_name,
        })
    })
    .transpose()
}

fn record_event(
    conn: &sql::Connection,
    user_id: &str,
    platform: ChannelPlatform,
    platform_user_id: &str,
    action: LinkAction,
) -> Result<()> {
    conn.execute(
        "INSERT INTO platform_link_event (id, user_id, platform, platform_user_id, action, at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            uuid::Uuid::new_v4().to_string(),
            user_id,
            platform.to_string(),
            platform_user_id,
            action.as_str(),
            crate::time::stored_now()
        ],
    )?;
    Ok(())
}

/// Forget what the person typed by hand for `platform`: the proven link replaces it (#82).
fn clear_hand_typed(conn: &sql::Connection, user_id: &str, platform: ChannelPlatform) -> Result<()> {
    let data = conn
        .query_row(
            "SELECT data FROM user_profile WHERE user_id = ?1",
            params![user_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let Some(data) = data else { return Ok(()) };
    let mut profile: UserProfile = serde_json::from_str(&data)?;
    if !profile.clear_platform(platform) {
        return Ok(());
    }
    conn.execute(
        "UPDATE user_profile SET data = ?2 WHERE user_id = ?1",
        params![user_id, serde_json::to_string(&profile)?],
    )?;
    Ok(())
}

#[async_trait::async_trait]
impl PlatformLinkStorage for SqlStorage {
    async fn issue_connect_code(
        &self,
        person: &PlatformPerson,
        code_hash: &str,
        now_ms: i64,
        expires_ms: i64,
    ) -> Result<()> {
        let conn = self.conn.lock()?;
        conn.execute(
            "INSERT INTO platform_connect_code (platform, platform_user_id, code_hash, \
                 platform_username, platform_display_name, created_at_ms, expires_at_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
             ON CONFLICT (platform, platform_user_id) DO UPDATE SET \
                 code_hash = excluded.code_hash, \
                 platform_username = excluded.platform_username, \
                 platform_display_name = excluded.platform_display_name, \
                 created_at_ms = excluded.created_at_ms, \
                 expires_at_ms = excluded.expires_at_ms",
            params![
                person.platform.to_string(),
                person.id,
                code_hash,
                person.username,
                person.display_name,
                now_ms,
                expires_ms
            ],
        )?;
        Ok(())
    }

    async fn peek_connect_code(
        &self,
        code_hash: &str,
        now_ms: i64,
    ) -> Result<Option<PlatformPerson>> {
        let conn = self.conn.lock()?;
        person_for_code(&conn, code_hash, now_ms)
    }

    async fn redeem_connect_code(
        &self,
        code_hash: &str,
        user_id: &str,
        now_ms: i64,
    ) -> Result<RedeemOutcome> {
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        let Some(person) = person_for_code(&tx, code_hash, now_ms)? else {
            return Ok(RedeemOutcome::Unavailable);
        };
        let platform = person.platform.to_string();
        let holder = query_link(
            &tx,
            "l.platform = ?1 AND l.platform_user_id = ?2",
            &platform,
            &person.id,
        )?;
        if holder.is_some_and(|link| link.user_id != user_id) {
            // Dropping the transaction leaves the code in place.
            return Ok(RedeemOutcome::LinkedElsewhere);
        }
        let taken = tx.execute(
            "DELETE FROM platform_connect_code WHERE code_hash = ?1 AND expires_at_ms > ?2",
            params![code_hash, now_ms],
        )?;
        if taken == 0 {
            return Ok(RedeemOutcome::Unavailable);
        }
        let previous = query_link(
            &tx,
            "l.user_id = ?1 AND l.platform = ?2",
            user_id,
            &platform,
        )?;
        tx.execute(
            "DELETE FROM platform_link WHERE user_id = ?1 AND platform = ?2",
            params![user_id, platform],
        )?;
        tx.execute(
            "INSERT INTO platform_link (platform, platform_user_id, user_id, platform_username, \
                 platform_display_name, linked_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                platform,
                person.id,
                user_id,
                person.username,
                person.display_name,
                crate::time::stored_now()
            ],
        )?;
        clear_hand_typed(&tx, user_id, person.platform)?;
        let previous = previous.filter(|old| old.platform_user_id != person.id);
        let action = if previous.is_some() {
            LinkAction::Replaced
        } else {
            LinkAction::Linked
        };
        record_event(&tx, user_id, person.platform, &person.id, action)?;
        let link = query_link(
            &tx,
            "l.user_id = ?1 AND l.platform = ?2",
            user_id,
            &platform,
        )?
        .context("the link just written is missing")?;
        tx.commit()?;
        Ok(match previous {
            Some(previous) => RedeemOutcome::Replaced { link, previous },
            None => RedeemOutcome::Linked(link),
        })
    }

    async fn find_link(
        &self,
        platform: ChannelPlatform,
        platform_user_id: &str,
    ) -> Result<Option<PlatformLink>> {
        let conn = self.conn.lock()?;
        query_link(
            &conn,
            "l.platform = ?1 AND l.platform_user_id = ?2",
            &platform.to_string(),
            platform_user_id,
        )
    }

    async fn list_links(&self, user_id: &str) -> Result<Vec<PlatformLink>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {LINK_COLUMNS} FROM {LINK_FROM} WHERE l.user_id = ?1 ORDER BY l.platform"
        ))?;
        let links: Result<Vec<_>> = stmt
            .query_map(params![user_id], read_link)?
            .map(|row| link_from(row?))
            .collect();
        links
    }

    async fn unlink_platform(&self, user_id: &str, platform: ChannelPlatform) -> Result<bool> {
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        let Some(link) = query_link(
            &tx,
            "l.user_id = ?1 AND l.platform = ?2",
            user_id,
            &platform.to_string(),
        )?
        else {
            return Ok(false);
        };
        tx.execute(
            "DELETE FROM platform_link WHERE user_id = ?1 AND platform = ?2",
            params![user_id, platform.to_string()],
        )?;
        record_event(&tx, user_id, platform, &link.platform_user_id, LinkAction::Unlinked)?;
        tx.commit()?;
        Ok(true)
    }

    async fn touch_link_names(&self, person: &PlatformPerson) -> Result<()> {
        let conn = self.conn.lock()?;
        conn.execute(
            "UPDATE platform_link SET platform_username = ?3, platform_display_name = ?4 \
             WHERE platform = ?1 AND platform_user_id = ?2",
            params![
                person.platform.to_string(),
                person.id,
                person.username,
                person.display_name
            ],
        )?;
        Ok(())
    }

    async fn list_link_events(&self, user_id: &str, limit: u32) -> Result<Vec<PlatformLinkEvent>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, user_id, platform, platform_user_id, action, at FROM platform_link_event \
             WHERE user_id = ?1 ORDER BY at DESC, id DESC LIMIT ?2",
        )?;
        let events: Result<Vec<_>> = stmt.query_map(params![user_id, i64::from(limit)], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?
        .map(|row| {
            let (id, user_id, platform, platform_user_id, action, at) = row?;
            Ok(PlatformLinkEvent {
                id,
                user_id,
                platform: parse_platform(&platform)?,
                platform_user_id,
                action: LinkAction::parse(&action)
                    .ok_or_else(|| anyhow::anyhow!("unknown link action {action:?}"))?,
                at: parse_time(&at)?,
            })
        })
        .collect();
        events
    }

    async fn purge_expired_connect_codes(&self, now_ms: i64) -> Result<u64> {
        let conn = self.conn.lock()?;
        let purged = conn.execute(
            "DELETE FROM platform_connect_code WHERE expires_at_ms <= ?1",
            params![now_ms],
        )?;
        Ok(purged as u64)
    }
}

#[cfg(test)]
#[path = "platform_link_tests.rs"]
mod tests;
