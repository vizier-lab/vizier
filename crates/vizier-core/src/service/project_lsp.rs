//! Language servers of a project over the service layer (`specs/054-project-lsp/contracts/http-api.md`):
//! the status, the per-language overrides and restart. Authorization is here; handlers translate.

use std::path::PathBuf;

use serde::Serialize;

use super::{
    ServiceError, Services,
    projects::{internal, invalid_code, project_for_manage, project_for_read},
};
use crate::{
    access::Caller,
    auth::Principal,
    projects::{access, lsp::languages},
    schema::{LspStatusSnapshot, ServerOverride},
    storage::project::ProjectStorage,
};

/// What the popup shows, plus what only a manager receives.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct LspView {
    #[serde(flatten)]
    pub snapshot: LspStatusSnapshot,
    pub can_manage: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub languages: Option<Vec<LanguageInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overrides: Option<Vec<OverrideInfo>>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct LanguageInfo {
    pub id: String,
    pub name: String,
    pub default_command: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct OverrideInfo {
    pub language: String,
    #[serde(flatten)]
    pub server: ServerOverride,
}

/// Body of `POST …/lsp/restart`.
#[derive(Debug, Clone, serde::Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct RestartRequest {
    pub language: String,
    /// A root already listed, relative to the project folder; without it every root restarts.
    #[serde(default)]
    pub root: Option<String>,
}

/// The snapshot a status-strip item and the inbox frame carry: no override values.
pub fn snapshot(s: &Services, project_id: &str, enabled: bool) -> LspStatusSnapshot {
    let lsp = &s.deps().lsp;
    LspStatusSnapshot {
        project_id: project_id.to_owned(),
        enabled,
        capacity: lsp.capacity(),
        servers: match enabled {
            true => lsp.status(project_id),
            false => vec![],
        },
    }
}

async fn view(
    s: &Services,
    who: &Principal,
    project: &crate::schema::Project,
) -> Result<LspView, ServiceError> {
    let can_manage = access::can_manage(Caller::from(who), project);
    let mut out = LspView {
        snapshot: snapshot(s, &project.id, project.lsp_enabled),
        can_manage,
        languages: None,
        overrides: None,
    };
    if can_manage {
        out.languages = Some(
            languages::LANGUAGES
                .iter()
                .map(|l| LanguageInfo {
                    id: l.id.into(),
                    name: l.name.into(),
                    default_command: l.command.into(),
                })
                .collect(),
        );
        out.overrides = Some(
            s.deps()
                .storage
                .lsp_overrides(&project.id)
                .await
                .map_err(internal)?
                .into_iter()
                .map(|(language, server)| OverrideInfo { language, server })
                .collect(),
        );
    }
    Ok(out)
}

/// `GET …/lsp`.
pub async fn status(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
) -> Result<LspView, ServiceError> {
    let project = project_for_read(s, who, agent_id, project_id).await?;
    view(s, who, &project).await
}

fn unknown_language(language: &str) -> ServiceError {
    ServiceError::Invalid {
        status: 404,
        message: format!("no built-in language named {language}"),
        code: Some("unknown_language".into()),
    }
}

/// The rules of `data-model.md` §2.
fn validate_override(server: &ServerOverride) -> Result<(), ServiceError> {
    let bad = |m: &str| invalid_code("invalid_override", m.to_owned());
    let command = server.command.trim();
    if command.is_empty() || command.len() > 1024 || command.contains(['\n', '\r', '\0']) {
        return Err(bad("the command must be 1 to 1,024 characters on one line"));
    }
    if server.args.len() > 32 || server.args.iter().any(|a| a.len() > 4096 || a.contains('\0')) {
        return Err(bad("at most 32 arguments"));
    }
    if server.env.len() > 32 {
        return Err(bad("at most 32 environment variables"));
    }
    for (name, value) in &server.env {
        let mut chars = name.chars();
        let valid = chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !valid || value.len() > 4096 || value.contains('\0') {
            return Err(bad("environment names look like NAME_1 and values are text"));
        }
    }
    Ok(())
}

/// `PUT …/lsp/overrides/{language}`: save, then stop that language's servers so the next use
/// starts the new one.
pub async fn set_override(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
    language: &str,
    mut server: ServerOverride,
) -> Result<LspView, ServiceError> {
    let project = project_for_manage(s, who, agent_id, project_id).await?;
    languages::language_by_id(language).ok_or_else(|| unknown_language(language))?;
    server.command = server.command.trim().to_owned();
    validate_override(&server)?;
    s.deps()
        .storage
        .set_lsp_override(
            project_id,
            language,
            &server,
            &who.user_id,
            crate::time::now().timestamp_millis(),
        )
        .await
        .map_err(internal)?;
    s.deps().lsp.stop_language(project_id, language).await;
    view(s, who, &project).await
}

/// `DELETE …/lsp/overrides/{language}`. Deleting an absent one succeeds.
pub async fn delete_override(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
    language: &str,
) -> Result<LspView, ServiceError> {
    let project = project_for_manage(s, who, agent_id, project_id).await?;
    languages::language_by_id(language).ok_or_else(|| unknown_language(language))?;
    s.deps()
        .storage
        .delete_lsp_override(project_id, language)
        .await
        .map_err(internal)?;
    s.deps().lsp.stop_language(project_id, language).await;
    view(s, who, &project).await
}

/// `POST …/lsp/restart`: both **Restart** and **Check again**. Only roots already listed.
pub async fn restart(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
    language: &str,
    root: Option<&str>,
) -> Result<LspView, ServiceError> {
    let project = project_for_manage(s, who, agent_id, project_id).await?;
    languages::language_by_id(language).ok_or_else(|| unknown_language(language))?;
    if !project.lsp_enabled {
        return Err(ServiceError::Conflict {
            message: "language servers are off for this project".into(),
            code: Some("lsp_disabled".into()),
            data: None,
        });
    }
    let root: Option<PathBuf> = match root {
        None | Some("") => None,
        Some(rel) => {
            let rel = std::path::Path::new(rel);
            if rel.is_absolute()
                || rel
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return Err(invalid_code("invalid_root", "the root must be inside the project"));
            }
            Some(project.folder.join(rel))
        }
    };
    s.deps()
        .lsp
        .restart(project_id, language, root.as_deref())
        .await;
    view(s, who, &project).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(command: &str) -> ServerOverride {
        ServerOverride {
            command: command.into(),
            ..Default::default()
        }
    }

    #[test]
    fn override_rules() {
        assert!(validate_override(&server("rust-analyzer")).is_ok());
        assert!(validate_override(&server("")).is_err());
        assert!(validate_override(&server("a\nb")).is_err());
        assert!(validate_override(&server(&"x".repeat(1025))).is_err());

        let mut many = server("x");
        many.args = vec!["a".into(); 33];
        assert!(validate_override(&many).is_err());

        let mut env = server("x");
        env.env.insert("OK_1".into(), "v".into());
        assert!(validate_override(&env).is_ok());
        env.env.insert("1BAD".into(), "v".into());
        assert!(validate_override(&env).is_err());
        let mut env = server("x");
        env.env.insert("has space".into(), "v".into());
        assert!(validate_override(&env).is_err());
    }
}

#[cfg(test)]
mod service_tests {
    use super::*;
    use crate::{
        auth::AuthMethod,
        config::VizierConfig,
        projects::lsp::{Availability, Target},
        service::projects::{self, OpenProjectRequest, UpdateProjectRequest},
        storage::{agent::AgentStorage, user::UserStorage},
    };

    struct World {
        services: Services,
        alice: Principal,
        bob: Principal,
        project_id: String,
        folder: tempfile::TempDir,
        _workspace: tempfile::TempDir,
    }

    async fn principal(deps: &crate::dependencies::VizierDependencies, name: &str, role_id: &str, role: &crate::storage::user::Role) -> Principal {
        deps.storage.create_user(name, "hash", role_id).await.unwrap();
        let stored = deps.storage.get_user(name).await.unwrap().unwrap();
        Principal {
            user_id: stored.user_id,
            username: name.into(),
            role: role.clone(),
            permissions: role.permissions.clone(),
            method: AuthMethod::Desktop,
        }
    }

    /// Alice manages everything; Bob can use the agent's projects but owns neither.
    async fn world() -> World {
        let workspace = tempfile::tempdir().unwrap();
        let deps = crate::dependencies::VizierDependencies::new(VizierConfig {
            workspace: workspace.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
        let manager = deps
            .storage
            .create_role("managers", vec!["all_agents:view".into(), access::USE_ALL.into()], false)
            .await
            .unwrap();
        let user = deps
            .storage
            .create_role("users", vec!["all_agents:view".into(), "owned_agents:projects".into()], false)
            .await
            .unwrap();
        let alice = principal(&deps, "alice", &manager.role_id, &manager).await;
        let mut bob = principal(&deps, "bob", &user.role_id, &user).await;
        let mut agent = crate::tests_support::agent_config("dp-a");
        agent.owner_id = Some(bob.user_id.clone());
        deps.storage.create_agent("dp-a", &agent).await.unwrap();
        bob.method = AuthMethod::Jwt;

        let services = Services::new(deps);
        let folder = tempfile::tempdir().unwrap();
        let opened = projects::open(
            &services,
            &alice,
            AuthMethod::Desktop,
            "dp-a",
            OpenProjectRequest {
                folder: folder.path().to_string_lossy().into_owned(),
                lsp_enabled: None,
            },
        )
        .await
        .unwrap();
        World { services, alice, bob, project_id: opened.project.id, folder, _workspace: workspace }
    }

    fn fake_target(world: &World) -> Target {
        let exe = std::env::current_exe().unwrap();
        let program = exe.parent().unwrap().parent().unwrap().join("examples").join("fake_lsp");
        Target {
            agent_id: "dp-a".into(),
            project_id: world.project_id.clone(),
            folder: world.folder.path().to_path_buf(),
            root: world.folder.path().to_path_buf(),
            language: languages::language_by_id("rust").unwrap(),
            server_override: Some(ServerOverride {
                command: program.to_string_lossy().into_owned(),
                args: vec!["--script".into(), "diagnostics".into()],
                env: Default::default(),
            }),
        }
    }

    #[tokio::test]
    async fn a_new_project_is_on_and_a_viewer_sees_neither_overrides_nor_defaults() {
        let w = world().await;
        let manager_view = status(&w.services, &w.alice, "dp-a", &w.project_id).await.unwrap();
        assert!(manager_view.snapshot.enabled);
        assert!(manager_view.can_manage);
        assert_eq!(manager_view.languages.as_ref().unwrap().len(), 16);
        assert!(manager_view.overrides.is_some());

        let viewer_view = status(&w.services, &w.bob, "dp-a", &w.project_id).await.unwrap();
        assert!(!viewer_view.can_manage);
        assert!(viewer_view.languages.is_none() && viewer_view.overrides.is_none());
        let json = serde_json::to_string(&viewer_view).unwrap();
        assert!(!json.contains("default_command") && !json.contains("overrides"));
    }

    #[tokio::test]
    async fn a_viewer_cannot_change_anything() {
        let w = world().await;
        let server = ServerOverride { command: "x".into(), ..Default::default() };
        let denied = |r: Result<LspView, ServiceError>| matches!(r, Err(ServiceError::Forbidden(_)));
        assert!(denied(set_override(&w.services, &w.bob, "dp-a", &w.project_id, "rust", server).await));
        assert!(denied(delete_override(&w.services, &w.bob, "dp-a", &w.project_id, "rust").await));
        assert!(denied(restart(&w.services, &w.bob, "dp-a", &w.project_id, "rust", None).await));
        let switch = projects::update(
            &w.services,
            &w.bob,
            "dp-a",
            &w.project_id,
            UpdateProjectRequest { name: None, shell_enabled: None, lsp_enabled: Some(false) },
        )
        .await;
        assert!(matches!(switch, Err(ServiceError::Forbidden(_))));
        // Nothing changed.
        assert!(status(&w.services, &w.alice, "dp-a", &w.project_id).await.unwrap().snapshot.enabled);
    }

    #[tokio::test]
    async fn overrides_are_validated_saved_and_removed() {
        let w = world().await;
        let unknown = set_override(&w.services, &w.alice, "dp-a", &w.project_id, "cobol",
            ServerOverride { command: "x".into(), ..Default::default() }).await;
        assert_eq!(unknown.unwrap_err().code(), Some("unknown_language"));
        let invalid = set_override(&w.services, &w.alice, "dp-a", &w.project_id, "rust",
            ServerOverride { command: "".into(), ..Default::default() }).await;
        assert_eq!(invalid.unwrap_err().code(), Some("invalid_override"));

        let saved = set_override(&w.services, &w.alice, "dp-a", &w.project_id, "rust",
            ServerOverride { command: "  /opt/ra  ".into(), args: vec!["-v".into()], env: [("RA_LOG".to_string(), "warn".to_string())].into() }).await.unwrap();
        let overrides = saved.overrides.unwrap();
        assert_eq!(overrides[0].language, "rust");
        assert_eq!(overrides[0].server.command, "/opt/ra");

        let removed = delete_override(&w.services, &w.alice, "dp-a", &w.project_id, "rust").await.unwrap();
        assert!(removed.overrides.unwrap().is_empty());
        // A retry is safe.
        delete_override(&w.services, &w.alice, "dp-a", &w.project_id, "rust").await.unwrap();
    }

    #[tokio::test]
    async fn switching_off_leaves_no_process_and_a_late_start_is_refused() {
        let w = world().await;
        let host = w.services.deps().lsp.clone();
        let target = fake_target(&w);
        host.ensure(&target);
        let began = std::time::Instant::now();
        let server = loop {
            if let Availability::Ready(server) = host.ensure(&target) {
                break server;
            }
            assert!(began.elapsed() < std::time::Duration::from_secs(15), "never ready");
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        };

        projects::update(
            &w.services,
            &w.alice,
            "dp-a",
            &w.project_id,
            UpdateProjectRequest { name: None, shell_enabled: None, lsp_enabled: Some(false) },
        )
        .await
        .unwrap();
        assert!(server.exit_info().is_some(), "the process is gone before the switch returns");
        assert!(host.status(&w.project_id).is_empty());
        // A turn that began before the switch now meets a closed project.
        assert!(matches!(host.ensure(&target), Availability::Closed));
        let view = status(&w.services, &w.alice, "dp-a", &w.project_id).await.unwrap();
        assert!(!view.snapshot.enabled && view.snapshot.servers.is_empty());
        // Restarting while off is a conflict.
        let r = restart(&w.services, &w.alice, "dp-a", &w.project_id, "rust", None).await;
        assert_eq!(r.unwrap_err().code(), Some("lsp_disabled"));

        // And back on.
        projects::update(
            &w.services,
            &w.alice,
            "dp-a",
            &w.project_id,
            UpdateProjectRequest { name: None, shell_enabled: None, lsp_enabled: Some(true) },
        )
        .await
        .unwrap();
        assert!(matches!(host.ensure(&target), Availability::Starting { .. }));
        host.stop_all().await;
    }

    #[tokio::test]
    async fn removing_the_project_stops_its_servers() {
        let w = world().await;
        let host = w.services.deps().lsp.clone();
        let target = fake_target(&w);
        host.ensure(&target);
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        projects::remove(&w.services, &w.alice, "dp-a", &w.project_id).await.unwrap();
        assert!(host.status(&w.project_id).is_empty());
        assert!(host.is_closed(&w.project_id));
    }
}
