//! `owner_fence` on both backends (`specs/057-fleet-row-ownership/contracts/ownership.md`): the
//! claim, and [`check_fence`], which every fenced write runs in its own transaction.

use anyhow::Result;
use tokio::sync::watch;
use vizier_schema::FencingToken;

use crate::storage::fence::{Fence, FenceStorage, Fences, OwnershipLost, Scope};
use crate::storage::sql::{Connection, params};
use crate::storage::sqlite::SqlStorage;

/// A Fleet claim: refused (no row returned) when a claim under a lease token at least as new
/// already succeeded.
const CLAIM_UNDER_LEASE: &str = "INSERT INTO owner_fence (scope, node, token, lease_token, claimed_at)
     VALUES (?1, ?2, 1, ?3, ?4)
     ON CONFLICT (scope) DO UPDATE SET
         token = owner_fence.token + 1,
         node = excluded.node,
         lease_token = excluded.lease_token,
         claimed_at = excluded.claimed_at
     WHERE owner_fence.lease_token IS NULL OR owner_fence.lease_token < excluded.lease_token
     RETURNING token";

/// A single-node claim: always succeeds, and clears the lease token, so a later Fleet claim of
/// any lease token succeeds too.
const CLAIM_SINGLE_NODE: &str = "INSERT INTO owner_fence (scope, node, token, lease_token, claimed_at)
     VALUES (?1, ?2, 1, NULL, ?3)
     ON CONFLICT (scope) DO UPDATE SET
         token = owner_fence.token + 1,
         node = excluded.node,
         lease_token = NULL,
         claimed_at = excluded.claimed_at
     RETURNING token";

/// Refuse a write of `scope` unless the fence this process holds for it is still current. Run it
/// inside the write's own transaction, before the write: on PostgreSQL the row is read
/// `FOR SHARE`, so a claim waits for the write to commit (research R2). A process that holds no
/// fence for `scope` (a gateway) is not an owner and is never refused (contract rule 5).
pub(crate) fn check_fence(conn: &Connection, fences: &Fences, scope: &Scope) -> Result<()> {
    let Some(held) = fences.held(scope) else {
        return Ok(());
    };
    let sql = format!(
        "SELECT token FROM owner_fence WHERE scope = ?1{}",
        conn.dialect().share_lock_suffix()
    );
    let stored = conn
        .query_all(&sql, params![scope.to_string()])?
        .first()
        .map(|row| row.get::<_, i64>(0))
        .transpose()?;
    if stored == Some(held as i64) {
        return Ok(());
    }
    fences.mark_lost(scope);
    Err(OwnershipLost {
        scope: scope.to_string(),
    }
    .into())
}

#[async_trait::async_trait]
impl FenceStorage for SqlStorage {
    async fn claim(&self, scope: &Scope, node: &str, lease: Option<FencingToken>) -> Result<Fence> {
        let key = scope.to_string();
        let now = crate::time::stored_now();
        let rows = {
            let conn = self.conn.lock()?;
            match lease {
                Some(FencingToken(lease)) => conn.query_all(
                    CLAIM_UNDER_LEASE,
                    params![key, node, lease as i64, now],
                )?,
                None => conn.query_all(CLAIM_SINGLE_NODE, params![key, node, now])?,
            }
        };
        let Some(row) = rows.first() else {
            return Err(OwnershipLost { scope: key }.into());
        };
        let fence = Fence {
            scope: scope.clone(),
            node: node.to_owned(),
            token: row.get::<_, i64>(0)? as u64,
        };
        self.fences.hold(fence.clone());
        Ok(fence)
    }

    fn ownership_lost(&self, scope: &Scope) -> watch::Receiver<bool> {
        self.fences.lost(scope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::fence::is_ownership_lost;
    use crate::storage::sql::SqlStore;
    use crate::tests_support::{node_storage, test_store_pair};

    fn sqlite() -> SqlStore {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_ownership_schema(&conn).unwrap();
        SqlStore::sqlite(conn)
    }

    fn token(storage: &SqlStorage, scope: &Scope) -> Option<i64> {
        storage
            .conn
            .lock()
            .unwrap()
            .query_all(
                "SELECT token FROM owner_fence WHERE scope = ?1",
                params![scope.to_string()],
            )
            .unwrap()
            .first()
            .map(|row| row.get::<_, i64>(0).unwrap())
    }

    #[tokio::test]
    async fn claim_is_monotonic_across_instances() {
        let (a, b) = test_store_pair(sqlite);
        let ((a, _da), (b, _db)) = (node_storage(a), node_storage(b));
        let scope = Scope::Agent("dp".into());
        let mut last = 0;
        for i in 0..6 {
            let owner = if i % 2 == 0 { &a } else { &b };
            let fence = owner.claim(&scope, "n", None).await.unwrap();
            assert!(fence.token > last, "{} after {last}", fence.token);
            last = fence.token;
        }
        assert_eq!(token(&a, &scope), Some(last as i64));
    }

    #[tokio::test]
    async fn late_claim_refused() {
        let (a, b) = test_store_pair(sqlite);
        let ((a, _da), (b, _db)) = (node_storage(a), node_storage(b));
        let scope = Scope::Control;
        let fence = a.claim(&scope, "a", Some(FencingToken(6))).await.unwrap();
        let err = b
            .claim(&scope, "b", Some(FencingToken(5)))
            .await
            .unwrap_err();
        assert!(is_ownership_lost(&err));
        assert_eq!(token(&a, &scope), Some(fence.token as i64));
        assert_eq!(b.fences.held(&scope), None, "a refused claim holds nothing");
        // An equal lease token is refused too: it is the same acquisition.
        assert!(b.claim(&scope, "b", Some(FencingToken(6))).await.is_err());
    }

    #[tokio::test]
    async fn single_node_after_fleet_and_back() {
        let (store, _) = test_store_pair(sqlite);
        let (storage, _dir) = node_storage(store);
        let scope = Scope::Agent("dp".into());
        let first = storage
            .claim(&scope, "n", Some(FencingToken(9)))
            .await
            .unwrap();
        let second = storage.claim(&scope, "n", None).await.unwrap();
        assert!(second.token > first.token);
        let third = storage
            .claim(&scope, "n", Some(FencingToken(1)))
            .await
            .unwrap();
        assert!(third.token > second.token);
    }

    #[tokio::test]
    async fn check_fence_unfenced_without_hold() {
        let (a, b) = test_store_pair(sqlite);
        let ((a, _da), (b, _db)) = (node_storage(a), node_storage(b));
        let scope = Scope::Agent("dp".into());
        a.claim(&scope, "a", None).await.unwrap();
        let conn = b.conn.lock().unwrap();
        check_fence(&conn, &b.fences, &scope).unwrap();
    }

    #[tokio::test]
    async fn check_fence_refuses_a_superseded_owner() {
        let (a, b) = test_store_pair(sqlite);
        let ((a, _da), (b, _db)) = (node_storage(a), node_storage(b));
        let scope = Scope::Agent("dp".into());
        a.claim(&scope, "a", None).await.unwrap();
        {
            let conn = a.conn.lock().unwrap();
            check_fence(&conn, &a.fences, &scope).unwrap();
        }
        let lost = a.ownership_lost(&scope);
        b.claim(&scope, "b", None).await.unwrap();
        let err = {
            let conn = a.conn.lock().unwrap();
            check_fence(&conn, &a.fences, &scope).unwrap_err()
        };
        assert!(is_ownership_lost(&err));
        assert!(*lost.borrow());
        let conn = b.conn.lock().unwrap();
        check_fence(&conn, &b.fences, &scope).unwrap();
    }

    crate::postgres_tests!(
        claim_is_monotonic_across_instances,
        late_claim_refused,
        single_node_after_fleet_and_back,
        check_fence_unfenced_without_hold,
        check_fence_refuses_a_superseded_owner,
    );
}
