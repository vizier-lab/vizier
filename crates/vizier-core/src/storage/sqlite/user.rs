use crate::storage::sql::params;
use anyhow::Result;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::storage::{
    sqlite::SqlStorage,
    user::{ApiKey, Role, User, UserProfile, UserStorage, Visit},
};

fn parse_time(text: Option<String>) -> Option<DateTime<Utc>> {
    text.and_then(|t| DateTime::parse_from_rfc3339(&t).ok())
        .map(|t| t.with_timezone(&Utc))
}

fn read_visit(conn: &crate::storage::sql::Connection, user_id: &str) -> Result<Visit> {
    let mut stmt =
        conn.prepare(r#"SELECT last_seen_at, previous_visit_at FROM "user" WHERE user_id = ?1"#)?;
    let mut rows = stmt.query_map(params![user_id], |row| {
        Ok((
            row.get::<_, Option<String>>(0)?,
            row.get::<_, Option<String>>(1)?,
        ))
    })?;
    Ok(match rows.next() {
        Some(row) => {
            let (last, previous) = row?;
            Visit {
                last_seen_at: parse_time(last),
                previous_visit_at: parse_time(previous),
            }
        }
        None => Visit::default(),
    })
}

#[async_trait::async_trait]
impl UserStorage for SqlStorage {
    async fn get_user(&self, username: &str) -> Result<Option<User>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(r#"SELECT data FROM "user" WHERE username = ?1"#)?;
        let mut rows = stmt.query_map(params![username], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })?;

        match rows.next() {
            Some(Ok(data)) => Ok(Some(serde_json::from_str(&data)?)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    async fn get_user_by_id(&self, user_id: &str) -> Result<Option<User>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(r#"SELECT data FROM "user" WHERE user_id = ?1"#)?;
        let mut rows = stmt.query_map(params![user_id], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })?;

        match rows.next() {
            Some(Ok(data)) => Ok(Some(serde_json::from_str(&data)?)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    async fn create_user(
        &self,
        username: &str,
        password_hash: &str,
        role_id: &str,
    ) -> Result<User> {
        let user = User {
            user_id: Uuid::new_v4().to_string(),
            username: username.to_string(),
            password_hash: password_hash.to_string(),
            role_id: role_id.to_string(),
            created_at: Utc::now(),
        };

        let data = serde_json::to_string(&user)?;
        let conn = self.conn.lock()?;
        conn.execute(
            r#"INSERT INTO "user" (user_id, username, data) VALUES (?1, ?2, ?3)"#,
            params![user.user_id, user.username, data],
        )?;

        Ok(user)
    }

    async fn update_user(
        &self,
        user_id: &str,
        username: Option<&str>,
        role_id: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock()?;
        let data: String = {
            let mut stmt = conn.prepare(r#"SELECT data FROM "user" WHERE user_id = ?1"#)?;
            let mut rows = stmt.query_map(params![user_id], |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?;
            match rows.next() {
                Some(Ok(d)) => d,
                _ => return Err(anyhow::anyhow!("User not found")),
            }
        };

        let mut user: User = serde_json::from_str(&data)?;
        if let Some(username) = username {
            user.username = username.to_string();
        }
        if let Some(role_id) = role_id {
            user.role_id = role_id.to_string();
        }

        let new_data = serde_json::to_string(&user)?;
        conn.execute(
            r#"UPDATE "user" SET data = ?1 WHERE user_id = ?2"#,
            params![new_data, user_id],
        )?;
        Ok(())
    }

    async fn delete_user(&self, user_id: &str) -> Result<()> {
        let conn = self.conn.lock()?;
        let deleted = conn.execute(r#"DELETE FROM "user" WHERE user_id = ?1"#, params![user_id])?;
        if deleted == 0 {
            return Err(anyhow::anyhow!("User not found"));
        }
        Ok(())
    }

    async fn list_users(&self) -> Result<Vec<User>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(r#"SELECT data FROM "user""#)?;
        let users = stmt
            .query_map([], |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<User>(&data).ok())
            .collect();
        Ok(users)
    }

    async fn update_password(&self, user_id: &str, password_hash: &str) -> Result<()> {
        let conn = self.conn.lock()?;
        let data: String = {
            let mut stmt = conn.prepare(r#"SELECT data FROM "user" WHERE user_id = ?1"#)?;
            let mut rows = stmt.query_map(params![user_id], |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?;
            match rows.next() {
                Some(Ok(d)) => d,
                _ => return Err(anyhow::anyhow!("User not found")),
            }
        };

        let mut user: User = serde_json::from_str(&data)?;
        user.password_hash = password_hash.to_string();

        let new_data = serde_json::to_string(&user)?;
        conn.execute(
            r#"UPDATE "user" SET data = ?1 WHERE user_id = ?2"#,
            params![new_data, user_id],
        )?;
        Ok(())
    }

    async fn record_visit(&self, user_id: &str, now: DateTime<Utc>) -> Result<Visit> {
        let conn = self.conn.lock()?;
        let visit = read_visit(&conn, user_id)?.seen_at(now);
        conn.execute(
            r#"UPDATE "user" SET last_seen_at = ?1, previous_visit_at = ?2 WHERE user_id = ?3"#,
            params![
                visit.last_seen_at.map(|t| t.to_rfc3339()),
                visit.previous_visit_at.map(|t| t.to_rfc3339()),
                user_id
            ],
        )?;
        Ok(visit)
    }

    async fn get_visit(&self, user_id: &str) -> Result<Visit> {
        read_visit(&self.conn.lock()?, user_id)
    }

    async fn user_exists(&self) -> Result<bool> {
        let conn = self.conn.lock()?;
        let count: i64 = conn.query_row(r#"SELECT COUNT(*) FROM "user""#, [], |row| row.get(0))?;
        Ok(count > 0)
    }

    async fn create_role(
        &self,
        name: &str,
        permissions: Vec<String>,
        is_system: bool,
    ) -> Result<Role> {
        let role = Role {
            role_id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            permissions,
            is_system,
            created_at: Utc::now(),
        };

        let data = serde_json::to_string(&role)?;
        let conn = self.conn.lock()?;
        conn.execute(
            "INSERT INTO role (role_id, is_system, data) VALUES (?1, ?2, ?3)",
            params![role.role_id, role.is_system as i32, data],
        )?;

        Ok(role)
    }

    async fn get_role(&self, role_id: &str) -> Result<Option<Role>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare("SELECT data FROM role WHERE role_id = ?1")?;
        let mut rows = stmt.query_map(params![role_id], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })?;

        match rows.next() {
            Some(Ok(data)) => Ok(Some(serde_json::from_str(&data)?)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    async fn list_roles(&self) -> Result<Vec<Role>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare("SELECT data FROM role")?;
        let roles = stmt
            .query_map([], |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<Role>(&data).ok())
            .collect();
        Ok(roles)
    }

    async fn update_role(&self, role_id: &str, name: &str, permissions: Vec<String>) -> Result<()> {
        let conn = self.conn.lock()?;
        let data: String = {
            let mut stmt = conn.prepare("SELECT data FROM role WHERE role_id = ?1")?;
            let mut rows = stmt.query_map(params![role_id], |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?;
            match rows.next() {
                Some(Ok(d)) => d,
                _ => return Err(anyhow::anyhow!("Role not found")),
            }
        };

        let mut role: Role = serde_json::from_str(&data)?;
        role.name = name.to_string();
        role.permissions = permissions;

        let new_data = serde_json::to_string(&role)?;
        conn.execute(
            "UPDATE role SET data = ?1 WHERE role_id = ?2",
            params![new_data, role_id],
        )?;
        Ok(())
    }

    async fn delete_role(&self, role_id: &str) -> Result<()> {
        let conn = self.conn.lock()?;
        let deleted = conn.execute("DELETE FROM role WHERE role_id = ?1", params![role_id])?;
        if deleted == 0 {
            return Err(anyhow::anyhow!("Role not found"));
        }
        Ok(())
    }

    async fn get_system_role(&self) -> Result<Option<Role>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare("SELECT data FROM role WHERE is_system = 1 LIMIT 1")?;
        let mut rows = stmt.query_map([], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })?;

        match rows.next() {
            Some(Ok(data)) => Ok(Some(serde_json::from_str(&data)?)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    async fn get_user_profile(&self, user_id: &str) -> Result<Option<UserProfile>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare("SELECT data FROM user_profile WHERE user_id = ?1")?;
        let mut rows = stmt.query_map(params![user_id], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })?;

        match rows.next() {
            Some(Ok(data)) => Ok(Some(serde_json::from_str(&data)?)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    async fn upsert_user_profile(&self, user_id: &str, profile: &UserProfile) -> Result<()> {
        let data = serde_json::to_string(profile)?;
        let conn = self.conn.lock()?;
        conn.execute(
            "INSERT INTO user_profile (user_id, data) VALUES (?1, ?2) \
             ON CONFLICT (user_id) DO UPDATE SET data = excluded.data",
            params![user_id, data],
        )?;
        Ok(())
    }

    async fn create_api_key(
        &self,
        user_id: &str,
        name: &str,
        key_hash: &str,
        prefix: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<ApiKey> {
        let api_key = ApiKey {
            prefix: Some(prefix.to_string()),
            id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            name: name.to_string(),
            key_hash: key_hash.to_string(),
            expires_at,
            created_at: Utc::now(),
            last_used_at: None,
        };

        let data = serde_json::to_string(&api_key)?;
        let conn = self.conn.lock()?;
        conn.execute(
            "INSERT INTO api_key (id, user_id, key_hash, data) VALUES (?1, ?2, ?3, ?4)",
            params![api_key.id, api_key.user_id, api_key.key_hash, data],
        )?;

        Ok(api_key)
    }

    async fn get_api_key_by_hash(&self, key_hash: &str) -> Result<Option<ApiKey>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare("SELECT data FROM api_key WHERE key_hash = ?1")?;
        let now = Utc::now();
        let keys: Vec<ApiKey> = stmt
            .query_map(params![key_hash], |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<ApiKey>(&data).ok())
            .filter(|k| k.expires_at.is_none_or(|exp| exp > now))
            .collect();

        Ok(keys.into_iter().next())
    }

    async fn list_api_keys(&self, user_id: &str) -> Result<Vec<ApiKey>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare("SELECT data FROM api_key WHERE user_id = ?1")?;
        let keys = stmt
            .query_map(params![user_id], |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<ApiKey>(&data).ok())
            .collect();
        Ok(keys)
    }

    async fn delete_api_key(&self, key_id: &str) -> Result<()> {
        let conn = self.conn.lock()?;
        conn.execute("DELETE FROM api_key WHERE id = ?1", params![key_id])?;
        Ok(())
    }

    async fn update_api_key_last_used(&self, key_id: &str) -> Result<()> {
        let conn = self.conn.lock()?;
        let data: String = {
            let mut stmt = conn.prepare("SELECT data FROM api_key WHERE id = ?1")?;
            let mut rows = stmt.query_map(params![key_id], |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?;
            match rows.next() {
                Some(Ok(d)) => d,
                _ => return Err(anyhow::anyhow!("API key not found")),
            }
        };

        let mut api_key: ApiKey = serde_json::from_str(&data)?;
        api_key.last_used_at = Some(Utc::now());

        let new_data = serde_json::to_string(&api_key)?;
        conn.execute(
            "UPDATE api_key SET data = ?1 WHERE id = ?2",
            params![new_data, key_id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::storage::{document::BlobDocumentStore, sql::SqlStore};

    fn setup() -> (SqlStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::tests_support::test_store(|| {
            let conn = rusqlite::Connection::open_in_memory().unwrap();
            SqlStorage::init_schema(&conn).unwrap();
            SqlStore::sqlite(conn)
        });
        let storage = SqlStorage::new(
            store,
            Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())),
        );
        (storage, dir)
    }

    #[tokio::test]
    async fn users_roles_and_visits_round_trip() {
        let (s, _dir) = setup();
        assert!(!s.user_exists().await.unwrap());
        let role = s
            .create_role("admin", vec!["*".into()], true)
            .await
            .unwrap();
        assert_eq!(
            s.get_system_role().await.unwrap().unwrap().role_id,
            role.role_id
        );

        let user = s.create_user("ana", "hash", &role.role_id).await.unwrap();
        assert!(s.user_exists().await.unwrap());
        assert!(
            s.create_user("ana", "other", &role.role_id).await.is_err(),
            "usernames are unique"
        );
        s.update_user(&user.user_id, Some("ana2"), None)
            .await
            .unwrap();
        s.update_password(&user.user_id, "hash2").await.unwrap();
        let got = s.get_user_by_id(&user.user_id).await.unwrap().unwrap();
        assert_eq!(got.username, "ana2");
        assert_eq!(got.password_hash, "hash2");
        assert_eq!(s.list_users().await.unwrap().len(), 1);

        assert_eq!(s.get_visit(&user.user_id).await.unwrap().last_seen_at, None);
        let now = Utc::now();
        let visit = s.record_visit(&user.user_id, now).await.unwrap();
        let stored = s.get_visit(&user.user_id).await.unwrap();
        assert_eq!(
            stored.last_seen_at.map(|t| t.timestamp_millis()),
            visit.last_seen_at.map(|t| t.timestamp_millis())
        );

        s.delete_user(&user.user_id).await.unwrap();
        assert!(s.get_user_by_id(&user.user_id).await.unwrap().is_none());
        assert!(s.delete_user(&user.user_id).await.is_err());
    }

    #[tokio::test]
    async fn profiles_upsert_and_api_keys_resolve_by_hash() {
        let (s, _dir) = setup();
        let mut profile = UserProfile {
            user_id: "u1".into(),
            discord_id: Some("d1".into()),
            discord_username: None,
            telegram_id: None,
            telegram_username: None,
            alias: vec![],
        };
        s.upsert_user_profile("u1", &profile).await.unwrap();
        profile.alias = vec!["annie".into()];
        s.upsert_user_profile("u1", &profile).await.unwrap();
        assert_eq!(
            s.get_user_profile("u1").await.unwrap().unwrap().alias,
            vec!["annie".to_string()]
        );

        let key = s
            .create_api_key("u1", "ci", "hash-1", "vz_1", None)
            .await
            .unwrap();
        assert_eq!(
            s.get_api_key_by_hash("hash-1").await.unwrap().unwrap().id,
            key.id
        );
        s.update_api_key_last_used(&key.id).await.unwrap();
        assert!(
            s.list_api_keys("u1").await.unwrap()[0]
                .last_used_at
                .is_some()
        );
        s.delete_api_key(&key.id).await.unwrap();
        assert!(s.list_api_keys("u1").await.unwrap().is_empty());
    }

    crate::postgres_tests!(
        users_roles_and_visits_round_trip,
        profiles_upsert_and_api_keys_resolve_by_hash,
    );
}
