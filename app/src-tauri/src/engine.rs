//! The embedded engine and the state machine around it (data-model.md, "Desktop app state").

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex as StdMutex},
};

use serde::Serialize;
use tokio::{sync::{Mutex, broadcast}, task::JoinHandle};
use vizier_core::{
    config::VizierConfig,
    runtime::{RuntimeEvent, RuntimeOptions, VizierRuntime},
    storage::user::{AVAILABLE_PERMISSIONS, UserStorage},
};
use vizier_http::embedded::Dispatcher;

use crate::{
    settings::{DesktopSettings, WebAccess},
    web_access::{self, WebAccessHandle, WebAccessStatus},
};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Candidate {
    pub user_id: String,
    pub username: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DesktopState {
    Starting,
    Locked { holder: String },
    NeedsOwner,
    ChooseOwner { candidates: Vec<Candidate> },
    Ready,
    Degraded { stopped: Vec<String> },
    Failed { error: String },
}

impl DesktopState {
    /// `{state, detail}` as `desktop_state` and the `desktop://state` event carry it.
    pub fn to_json(&self) -> serde_json::Value {
        use serde_json::json;
        let (state, detail) = match self {
            Self::Starting => ("starting", json!(null)),
            Self::Locked { holder } => ("locked", json!({ "holder": holder })),
            Self::NeedsOwner => ("needs_owner", json!(null)),
            Self::ChooseOwner { candidates } => ("choose_owner", json!({ "candidates": candidates })),
            Self::Ready => ("ready", json!(null)),
            Self::Degraded { stopped } => ("degraded", json!({ "stopped": stopped })),
            Self::Failed { error } => ("failed", json!({ "error": error })),
        };
        json!({ "state": state, "detail": detail })
    }

    pub fn is_serving(&self) -> bool {
        matches!(self, Self::Ready | Self::Degraded { .. })
    }
}

/// An error a command reports with a stable `code`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl CommandError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into() }
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

#[derive(Default)]
struct Running {
    runtime: Option<VizierRuntime>,
    dispatcher: Option<Arc<Dispatcher>>,
    owner: Option<String>,
    web: Option<WebAccessHandle>,
    monitor: Option<JoinHandle<()>>,
}

struct Inner {
    config_dir: PathBuf,
    settings: StdMutex<DesktopSettings>,
    state: StdMutex<DesktopState>,
    state_tx: broadcast::Sender<DesktopState>,
    web_tx: broadcast::Sender<WebAccessStatus>,
    running: Mutex<Running>,
}

#[derive(Clone)]
pub struct Engine {
    inner: Arc<Inner>,
}

fn canonical(path: &Path) -> String {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()).to_string_lossy().to_string()
}

impl Engine {
    pub fn new(config_dir: PathBuf, settings: DesktopSettings) -> Self {
        let (state_tx, _) = broadcast::channel(32);
        let (web_tx, _) = broadcast::channel(32);
        Self {
            inner: Arc::new(Inner {
                config_dir,
                settings: StdMutex::new(settings),
                state: StdMutex::new(DesktopState::Starting),
                state_tx,
                web_tx,
                running: Mutex::new(Running::default()),
            }),
        }
    }

    pub fn state(&self) -> DesktopState {
        self.inner.state.lock().unwrap().clone()
    }

    pub fn subscribe_state(&self) -> broadcast::Receiver<DesktopState> {
        self.inner.state_tx.subscribe()
    }

    pub fn subscribe_web_access(&self) -> broadcast::Receiver<WebAccessStatus> {
        self.inner.web_tx.subscribe()
    }

    pub fn settings(&self) -> DesktopSettings {
        self.inner.settings.lock().unwrap().clone()
    }

    fn set_state(&self, state: DesktopState) {
        *self.inner.state.lock().unwrap() = state.clone();
        let _ = self.inner.state_tx.send(state);
    }

    fn save_settings(&self, edit: impl FnOnce(&mut DesktopSettings)) {
        let mut settings = self.inner.settings.lock().unwrap();
        edit(&mut settings);
        if let Err(e) = settings.save(&self.inner.config_dir) {
            tracing::warn!("cannot save desktop.json: {e}");
        }
    }

    /// The dispatcher once the engine is serving; every `api`/`topic`/asset call goes here.
    pub async fn dispatcher(&self) -> Option<Arc<Dispatcher>> {
        if !self.state().is_serving() {
            return None;
        }
        self.inner.running.lock().await.dispatcher.clone()
    }

    /// Starts the engine on the settings' workspace.
    pub async fn start(&self) {
        let workspace = self.settings().workspace;
        self.start_on(&workspace).await;
    }

    async fn start_on(&self, workspace: &Path) {
        self.set_state(DesktopState::Starting);
        let mut running = self.inner.running.lock().await;
        if let Err(e) = std::fs::create_dir_all(workspace) {
            self.set_state(DesktopState::Failed { error: format!("cannot create {}: {e}", workspace.display()) });
            return;
        }
        let mut config = VizierConfig::with_workspace(workspace.to_path_buf());
        // The key doubles as the JWT secret of the opt-in web listener (research R10); it is
        // part of the runtime's config because the router reads it from there.
        match web_access::ensure_key(workspace) {
            Ok(key) => {
                if let Some(http) = config.channels.http.as_mut() {
                    http.jwt_secret = web_access::hex(&key);
                }
            }
            Err(e) => {
                self.set_state(DesktopState::Failed { error: e });
                return;
            }
        }
        let runtime = match VizierRuntime::start(RuntimeOptions { config }).await {
            Ok(rt) => rt,
            Err(e) => {
                let message = e.0;
                if message.contains("already in use") {
                    self.set_state(DesktopState::Locked { holder: message });
                } else {
                    self.set_state(DesktopState::Failed { error: message });
                }
                return;
            }
        };
        running.monitor = Some(self.monitor(&runtime));
        running.runtime = Some(runtime);
        self.resolve_owner(&mut running, workspace).await;
    }

    fn monitor(&self, runtime: &VizierRuntime) -> JoinHandle<()> {
        let mut events = runtime.events();
        let engine = self.clone();
        tokio::spawn(async move {
            while let Ok(event) = events.recv().await {
                if let RuntimeEvent::SubsystemStopped { name, .. } = event {
                    let mut stopped = match engine.state() {
                        DesktopState::Degraded { stopped } => stopped,
                        DesktopState::Ready => vec![],
                        _ => continue,
                    };
                    stopped.push(name.to_string());
                    engine.set_state(DesktopState::Degraded { stopped });
                }
            }
        })
    }

    /// Decides which user the app signs in as (data-model.md state machine).
    async fn resolve_owner(&self, running: &mut Running, workspace: &Path) {
        let Some(runtime) = running.runtime.as_ref() else { return };
        let storage = runtime.deps().storage.clone();
        let users = match storage.list_users().await {
            Ok(u) => u,
            Err(e) => return self.set_state(DesktopState::Failed { error: e.to_string() }),
        };
        if users.is_empty() {
            return self.set_state(DesktopState::NeedsOwner);
        }
        let key = canonical(workspace);
        let bound = self.settings().bindings.get(&key).cloned();
        if let Some(id) = bound.filter(|id| users.iter().any(|u| &u.user_id == id)) {
            return self.bind(running, &key, id).await;
        }
        let mut system = vec![];
        for user in &users {
            if matches!(storage.get_role(&user.role_id).await, Ok(Some(r)) if r.is_system) {
                system.push(user.clone());
            }
        }
        match system.as_slice() {
            [only] => {
                let id = only.user_id.clone();
                self.bind(running, &key, id).await
            }
            _ => {
                let pool = if system.is_empty() { users } else { system };
                let candidates = pool
                    .into_iter()
                    .map(|u| Candidate { user_id: u.user_id, username: u.username })
                    .collect();
                self.set_state(DesktopState::ChooseOwner { candidates });
            }
        }
    }

    async fn bind(&self, running: &mut Running, workspace_key: &str, user_id: String) {
        let Some(runtime) = running.runtime.as_ref() else { return };
        self.save_settings(|s| {
            s.bindings.insert(workspace_key.to_string(), user_id.clone());
        });
        running.dispatcher = Some(Arc::new(Dispatcher::new(runtime, user_id.clone())));
        running.owner = Some(user_id);
        self.set_state(DesktopState::Ready);
        self.apply_web_access(running).await;
    }

    pub async fn setup_owner(&self, username: &str, password: Option<&str>) -> Result<(), CommandError> {
        let mut running = self.inner.running.lock().await;
        if self.state() != DesktopState::NeedsOwner {
            return Err(CommandError::new("invalid_state", "an owner can only be set up on an empty workspace"));
        }
        let username = username.trim();
        if username.is_empty() {
            return Err(CommandError::new("invalid_username", "a name is required"));
        }
        let runtime = running.runtime.as_ref().ok_or_else(|| CommandError::new("engine_not_ready", "engine is not running"))?;
        let storage = runtime.deps().storage.clone();
        let internal = |e: &dyn std::fmt::Display| CommandError::new("internal", e.to_string());
        let role = match storage.get_system_role().await.map_err(|e| internal(&e))? {
            Some(role) => role,
            None => storage
                .create_role(
                    "superadmin",
                    AVAILABLE_PERMISSIONS.iter().map(|p| p.to_string()).collect(),
                    true,
                )
                .await
                .map_err(|e| internal(&e))?,
        };
        // The owner can open the app's web access and sign in from a browser, so a password is
        // part of creating them.
        let password = password.unwrap_or_default();
        if password.is_empty() {
            return Err(CommandError::new("password_required", "a password is required"));
        }
        if password.chars().count() < MIN_PASSWORD_LEN {
            return Err(CommandError::new(
                "password_too_short",
                format!("the password needs at least {MIN_PASSWORD_LEN} characters"),
            ));
        }
        let hash = vizier_core::auth::password::hash(password).map_err(|e| internal(&e.0))?;
        let user = storage
            .create_user(username, &hash, &role.role_id)
            .await
            .map_err(|e| internal(&e))?;
        let key = canonical(Path::new(&runtime.deps().config.workspace));
        self.bind(&mut running, &key, user.user_id).await;
        Ok(())
    }

    pub async fn choose_owner(&self, user_id: &str) -> Result<(), CommandError> {
        let mut running = self.inner.running.lock().await;
        let DesktopState::ChooseOwner { candidates } = self.state() else {
            return Err(CommandError::new("invalid_state", "no owner choice is pending"));
        };
        if !candidates.iter().any(|c| c.user_id == user_id) {
            return Err(CommandError::new("not_a_candidate", "that user cannot be chosen"));
        }
        let Some(runtime) = running.runtime.as_ref() else {
            return Err(CommandError::new("engine_not_ready", "engine is not running"));
        };
        let key = canonical(Path::new(&runtime.deps().config.workspace));
        self.bind(&mut running, &key, user_id.to_string()).await;
        Ok(())
    }

    async fn stop(&self, running: &mut Running) {
        if let Some(web) = running.web.take() {
            web.stop();
        }
        if let Some(monitor) = running.monitor.take() {
            monitor.abort();
        }
        running.dispatcher = None;
        running.owner = None;
        if let Some(runtime) = running.runtime.take() {
            let _ = runtime.shutdown().await;
        }
    }

    pub async fn shutdown(&self) {
        let mut running = self.inner.running.lock().await;
        self.stop(&mut running).await;
    }

    pub async fn restart(&self) {
        self.shutdown().await;
        self.start().await;
    }

    pub async fn open_workspace(&self, path: PathBuf) {
        self.shutdown().await;
        self.save_settings(|s| s.workspace = path);
        self.start().await;
    }

    /// The bound owner's user id, when there is one.
    #[doc(hidden)]
    pub async fn owner_id(&self) -> Option<String> {
        self.inner.running.lock().await.owner.clone()
    }

    /// The running engine's storage, for tests that need to set up an older workspace's state.
    #[doc(hidden)]
    pub async fn storage_for_test(&self) -> Arc<vizier_core::storage::VizierStorage> {
        let running = self.inner.running.lock().await;
        running.runtime.as_ref().expect("engine is running").deps().storage.clone()
    }

    /// Whether the bound user can sign in from a browser: they have a password. (An owner of an
    /// older workspace may still have none.)
    pub async fn web_access_allowed(&self) -> bool {
        let running = self.inner.running.lock().await;
        let (Some(runtime), Some(owner)) = (running.runtime.as_ref(), running.owner.as_ref()) else {
            return false;
        };
        let storage = &runtime.deps().storage;
        let has_password = matches!(storage.get_user_by_id(owner).await, Ok(Some(u)) if vizier_core::auth::password::has_password(&u));
        has_password
    }

    /// Applies the settings a person may change; `workspace` and `bindings` are not theirs.
    pub async fn settings_set(&self, patch: SettingsPatch) -> Result<DesktopSettings, CommandError> {
        if patch.web_access.enabled && !self.web_access_allowed().await {
            return Err(CommandError::new(
                "password_required",
                "set a password before turning on web access",
            ));
        }
        self.save_settings(|s| {
            s.close_to_tray = patch.close_to_tray;
            s.web_access = patch.web_access;
            s.updates = patch.updates;
        });
        let mut running = self.inner.running.lock().await;
        self.apply_web_access(&mut running).await;
        Ok(self.settings())
    }

    async fn apply_web_access(&self, running: &mut Running) {
        if let Some(web) = running.web.take() {
            web.stop();
        }
        let settings = self.settings();
        let status = if settings.web_access.enabled && self.state().is_serving() {
            let Some(runtime) = running.runtime.as_ref() else { return };
            match web_access::start(runtime, &settings.web_access, self.inner.web_tx.clone()) {
                Ok(handle) => {
                    let status = WebAccessStatus::running(handle.url.clone());
                    running.web = Some(handle);
                    status
                }
                Err(error) => WebAccessStatus::failed(error),
            }
        } else {
            WebAccessStatus::stopped()
        };
        let _ = self.inner.web_tx.send(status);
    }

    pub async fn run_debug_stop(&self, name: &'static str) -> bool {
        let running = self.inner.running.lock().await;
        running.runtime.as_ref().is_some_and(|r| r.stop_subsystem_for_test(name))
    }
}

/// The same minimum the WebUI's password form uses.
const MIN_PASSWORD_LEN: usize = 8;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SettingsPatch {
    pub close_to_tray: bool,
    pub web_access: WebAccess,
    pub updates: crate::settings::Updates,
}
