//! The one Workspace permission matrix (`specs/053-workspace-channel/contracts/http-api.md`).
//! No handler or UI helper decides this on its own; routes only translate the result.
//!
//! Everything comes from membership, except two permissions: `workspaces:create`, and
//! `all_workspaces:manage`, which reads, posts in and manages every Workspace without being a
//! member. Someone who is neither a member nor a manager has **no standing at all**, and every
//! route answers `404` for them, so a Workspace's existence does not leak (FR-004).

use crate::{
    access::Caller,
    schema::{MemberKind, WorkspaceMember, WorkspaceRole},
};

/// May create Workspaces.
pub const CREATE: &str = "workspaces:create";
/// May read, post in and manage every Workspace.
pub const MANAGE_ALL: &str = "all_workspaces:manage";

/// Where a person stands in one Workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Member,
    Owner,
    /// Not a member, but holds `all_workspaces:manage`: treated as a member with management
    /// rights. They are not in the member list or the mention menu until added.
    Admin,
}

impl Standing {
    /// Read, post, stop turns, answer questions, add topics, run commands.
    pub fn can_participate(self) -> bool {
        true
    }

    /// Rename, delete, manage members, delete topics.
    pub fn can_manage(self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }

    /// Remove or demote the last owner: only a manager of everything may.
    pub fn can_orphan(self) -> bool {
        matches!(self, Self::Admin)
    }
}

/// May the person create a Workspace.
pub fn can_create(caller: Caller<'_>) -> bool {
    caller.is_system || caller.has(CREATE)
}

/// May the person manage every Workspace.
pub fn can_manage_all(caller: Caller<'_>) -> bool {
    caller.is_system || caller.has(MANAGE_ALL)
}

/// The person's standing in a Workspace, given their own membership row if they have one.
/// `None` means no access: the route must answer `404`.
pub fn standing(
    caller: Caller<'_>,
    username: &str,
    membership: Option<&WorkspaceMember>,
) -> Option<Standing> {
    match membership {
        Some(m) if m.kind == MemberKind::Person && m.handle == username => Some(match m.role {
            WorkspaceRole::Owner => Standing::Owner,
            WorkspaceRole::Member => Standing::Member,
        }),
        _ if can_manage_all(caller) => Some(Standing::Admin),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caller<'a>(username: &'a str, system: bool, permissions: &'a [String]) -> Caller<'a> {
        Caller { user_id: username, is_system: system, permissions }
    }

    fn member(handle: &str, role: WorkspaceRole) -> WorkspaceMember {
        WorkspaceMember {
            workspace_id: "w".into(),
            kind: MemberKind::Person,
            handle: handle.into(),
            role,
            added_by: "x".into(),
            added_at: 0,
        }
    }

    fn perms(list: &[&str]) -> Vec<String> {
        list.iter().map(|p| p.to_string()).collect()
    }

    #[test]
    fn a_stranger_has_no_standing() {
        let p = perms(&[]);
        assert_eq!(standing(caller("eve", false, &p), "eve", None), None);
        // Another person's membership row is not theirs.
        assert_eq!(standing(caller("eve", false, &p), "eve", Some(&member("rafi", WorkspaceRole::Owner))), None);
        // The create permission opens no existing Workspace.
        let creator = perms(&[CREATE]);
        assert_eq!(standing(caller("eve", false, &creator), "eve", None), None);
    }

    #[test]
    fn a_member_participates_but_does_not_manage() {
        let p = perms(&[]);
        let s = standing(caller("maya", false, &p), "maya", Some(&member("maya", WorkspaceRole::Member))).unwrap();
        assert_eq!(s, Standing::Member);
        assert!(s.can_participate() && !s.can_manage() && !s.can_orphan());
    }

    #[test]
    fn an_owner_manages_but_cannot_orphan_the_workspace() {
        let p = perms(&[]);
        let s = standing(caller("rafi", false, &p), "rafi", Some(&member("rafi", WorkspaceRole::Owner))).unwrap();
        assert_eq!(s, Standing::Owner);
        assert!(s.can_manage() && !s.can_orphan());
    }

    #[test]
    fn the_manage_all_permission_and_the_system_role_stand_in_every_workspace() {
        let p = perms(&[MANAGE_ALL]);
        let admin = standing(caller("ops", false, &p), "ops", None).unwrap();
        assert_eq!(admin, Standing::Admin);
        assert!(admin.can_manage() && admin.can_orphan());

        let none = perms(&[]);
        assert_eq!(standing(caller("root", true, &none), "root", None), Some(Standing::Admin));
    }

    #[test]
    fn a_member_who_also_manages_all_keeps_their_own_role() {
        let p = perms(&[MANAGE_ALL]);
        let s = standing(caller("maya", false, &p), "maya", Some(&member("maya", WorkspaceRole::Member))).unwrap();
        assert_eq!(s, Standing::Member);
    }

    #[test]
    fn creating_needs_the_permission_or_the_system_role() {
        let none = perms(&[]);
        let creator = perms(&[CREATE]);
        let manager_only = perms(&[MANAGE_ALL]);
        assert!(!can_create(caller("a", false, &none)));
        assert!(can_create(caller("a", false, &creator)));
        assert!(!can_create(caller("a", false, &manager_only)));
        assert!(can_create(caller("root", true, &none)));
    }
}
