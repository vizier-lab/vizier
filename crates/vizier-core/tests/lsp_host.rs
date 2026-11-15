//! The language-server host against the scriptable `fake_lsp` example (`specs/054-project-lsp`).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use vizier_core::projects::lsp::languages::language_by_id;
use vizier_core::projects::lsp::diagnostics::wait_after_write;
use vizier_core::projects::lsp::server::{RequestError, Server, ServerSpec};
use vizier_core::projects::lsp::{Availability, LspHost, Target};
use vizier_core::schema::ServerOverride;

fn fake_lsp() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let names: &[&str] = if cfg!(windows) { &["fake_lsp.exe", "fake_lsp"] } else { &["fake_lsp"] };
    // The binary sits in `target/debug/deps/` on an older cargo and in
    // `target/debug/build/<package>/<hash>/` on a newer one: look in every ancestor.
    let path = exe
        .ancestors()
        .skip(1)
        .flat_map(|dir| names.iter().map(move |name| dir.join("examples").join(name)))
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| exe.parent().unwrap().parent().unwrap().join("examples").join(names[0]));
    assert!(path.exists(), "build the example first: {}", path.display());
    path
}

struct Fixture {
    dir: tempfile::TempDir,
    /// Logs live outside the project folder, so writing them is not a change to the project.
    logs: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        Self { dir: tempfile::tempdir().unwrap(), logs: tempfile::tempdir().unwrap() }
    }

    fn log(&self, name: &str) -> PathBuf {
        self.logs.path().join(format!("{name}.log"))
    }

    fn target(&self, project: &str, script: &str, log: &str, extra: &[&str]) -> Target {
        let mut args: Vec<String> = vec![
            "--script".into(),
            script.into(),
            "--log".into(),
            self.log(log).to_string_lossy().into_owned(),
        ];
        args.extend(extra.iter().map(|s| s.to_string()));
        Target {
            agent_id: "agent".into(),
            project_id: project.into(),
            folder: self.dir.path().to_path_buf(),
            root: self.dir.path().to_path_buf(),
            language: language_by_id("rust").unwrap(),
            server_override: Some(ServerOverride {
                command: fake_lsp().to_string_lossy().into_owned(),
                args,
                env: Default::default(),
            }),
        }
    }
}

async fn until<F: Fn() -> bool>(limit: Duration, check: F) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if check() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    check()
}

fn state_of(host: &LspHost, project: &str) -> String {
    host.status(project)
        .first()
        .map(|s| s.state.clone())
        .unwrap_or_else(|| "none".into())
}

fn log_lines(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

#[tokio::test]
async fn handshake_then_ready_after_quiet() {
    let f = Fixture::new();
    let host = LspHost::new(6);
    let t = f.target("p1", "diagnostics", "a", &[]);
    assert!(matches!(host.ensure(&t), Availability::Starting { .. }));
    assert!(until(Duration::from_secs(10), || state_of(&host, "p1") == "ready").await);
    assert!(matches!(host.ensure(&t), Availability::Ready(_)));
    let methods: Vec<String> = log_lines(&f.log("a"))
        .iter()
        .filter_map(|m| m["method"].as_str().map(str::to_owned))
        .collect();
    assert_eq!(methods.first().map(String::as_str), Some("initialize"));
    assert!(methods.contains(&"initialized".to_string()));
    host.stop_all().await;
}

#[tokio::test]
async fn progress_keeps_it_starting_with_detail() {
    let f = Fixture::new();
    let host = LspHost::new(6);
    let t = f.target("p1", "progress", "a", &["--progress-ms", "2500"]);
    host.ensure(&t);
    assert!(
        until(Duration::from_secs(5), || {
            host.status("p1")
                .first()
                .is_some_and(|s| s.state == "starting" && s.detail.as_deref().is_some_and(|d| d.starts_with("indexing")))
        })
        .await,
        "{:?}",
        host.status("p1")
    );
    assert!(until(Duration::from_secs(12), || state_of(&host, "p1") == "ready").await);
    host.stop_all().await;
}

#[tokio::test]
async fn a_slow_start_does_not_block_ensure() {
    let f = Fixture::new();
    let host = LspHost::new(6);
    let t = f.target("p1", "slow-start", "a", &[]);
    let began = Instant::now();
    host.ensure(&t);
    assert!(began.elapsed() < Duration::from_millis(500));
    assert_eq!(state_of(&host, "p1"), "starting");
    host.stop_all().await;
}

#[tokio::test]
async fn a_server_that_keeps_crashing_ends_failed() {
    let f = Fixture::new();
    let host = LspHost::new(6);
    host.ensure(&f.target("p1", "crash", "a", &[]));
    assert!(until(Duration::from_secs(30), || state_of(&host, "p1") == "failed").await);
    host.stop_all().await;
}

#[tokio::test]
async fn exiting_before_initialize_with_a_not_installed_message_reads_missing() {
    let f = Fixture::new();
    let host = LspHost::new(6);
    host.ensure(&f.target("p1", "no-init-exit", "a", &[]));
    assert!(until(Duration::from_secs(10), || state_of(&host, "p1") == "missing").await);
    let status = &host.status("p1")[0];
    assert!(status.install_hint.as_deref().unwrap().contains("rustup"));
    assert_eq!(host.capacity().in_use, 0, "a missing entry holds no slot");
    host.stop_all().await;
}

#[tokio::test]
async fn a_command_that_does_not_exist_reads_missing_naming_it() {
    let f = Fixture::new();
    let host = LspHost::new(6);
    let mut t = f.target("p1", "x", "a", &[]);
    t.server_override.as_mut().unwrap().command = "definitely-not-a-real-server".into();
    host.ensure(&t);
    assert!(until(Duration::from_secs(10), || state_of(&host, "p1") == "missing").await);
    assert!(host.status("p1")[0].reason.as_deref().unwrap().contains("definitely-not-a-real-server"));
}

#[tokio::test]
async fn a_hung_server_only_affects_its_own_calls() {
    let f = Fixture::new();
    let host = LspHost::new(6);
    host.ensure(&f.target("hung", "hang", "h", &[]));
    host.ensure(&f.target("fine", "answers", "g", &[]));
    assert!(until(Duration::from_secs(10), || state_of(&host, "hung") == "ready"
        && state_of(&host, "fine") == "ready")
    .await);
    let (Availability::Ready(hung), Availability::Ready(fine)) = (
        host.ensure(&f.target("hung", "hang", "h", &[])),
        host.ensure(&f.target("fine", "answers", "g", &[])),
    ) else {
        panic!("both ready");
    };
    let params = json!({"textDocument": {"uri": "file:///x.rs"}, "position": {"line": 0, "character": 0}});
    let pending = {
        let params = params.clone();
        tokio::spawn(async move { hung.request("textDocument/hover", params).await })
    };
    let answer = fine.request("textDocument/hover", params).await.unwrap();
    assert!(answer["contents"]["value"].as_str().unwrap().contains("resolve"));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!pending.is_finished(), "the hung call is still waiting on its own server");
    pending.abort();
    host.stop_all().await;
}

#[tokio::test]
async fn the_environment_holds_no_secrets() {
    unsafe {
        std::env::set_var("FAKE_PROVIDER_API_KEY", "leak-me");
        std::env::set_var("VIZIER_JWT_SECRET", "leak-me");
        std::env::set_var("KEEP_ME_VISIBLE", "yes");
    }
    let f = Fixture::new();
    let host = LspHost::new(6);
    host.ensure(&f.target("p1", "dump-env", "a", &[]));
    assert!(until(Duration::from_secs(10), || state_of(&host, "p1") == "ready").await);
    let env = log_lines(&f.log("a"))
        .into_iter()
        .find_map(|l| l.get("env").cloned())
        .expect("env logged");
    assert!(env.get("FAKE_PROVIDER_API_KEY").is_none());
    assert!(env.get("VIZIER_JWT_SECRET").is_none());
    assert_eq!(env["KEEP_ME_VISIBLE"], "yes");
    host.stop_all().await;
}

#[tokio::test]
async fn an_edit_request_from_the_server_is_refused() {
    let f = Fixture::new();
    let host = LspHost::new(6);
    host.ensure(&f.target("p1", "apply-edit", "a", &[]));
    assert!(until(Duration::from_secs(10), || {
        log_lines(&f.log("a")).iter().any(|l| l["id"] == "e1" && l["result"]["applied"] == false)
    })
    .await);
    host.stop_all().await;
}

#[tokio::test]
async fn stop_all_leaves_nothing_running() {
    let f = Fixture::new();
    let host = LspHost::new(6);
    host.ensure(&f.target("p1", "diagnostics", "a", &[]));
    host.ensure(&f.target("p2", "pull", "b", &[]));
    assert!(until(Duration::from_secs(10), || state_of(&host, "p1") == "ready"
        && state_of(&host, "p2") == "ready")
    .await);
    let Availability::Ready(server) = host.ensure(&f.target("p1", "diagnostics", "a", &[])) else {
        panic!("ready");
    };
    let began = Instant::now();
    host.stop_all().await;
    assert!(began.elapsed() < Duration::from_secs(6));
    assert!(host.status("p1").is_empty() && host.status("p2").is_empty());
    assert!(server.exit_info().is_some_and(|e| e.requested));
    assert_eq!(host.capacity().in_use, 0);
    let r = server.request("textDocument/hover", json!({})).await;
    assert!(matches!(r, Err(RequestError::Exited(_)) | Err(RequestError::Timeout)));
}

#[tokio::test]
async fn a_closed_project_starts_nothing() {
    let f = Fixture::new();
    let host = LspHost::new(6);
    host.close_project("p1").await;
    assert!(matches!(host.ensure(&f.target("p1", "diagnostics", "a", &[])), Availability::Closed));
    assert!(host.status("p1").is_empty());
    host.open_project("p1");
    assert!(matches!(host.ensure(&f.target("p1", "diagnostics", "a", &[])), Availability::Starting { .. }));
    host.stop_all().await;
}

// ---------------------------------------------------------------- Story 1: diagnostics

async fn start_direct(f: &Fixture, script: &str, extra: &[&str]) -> std::sync::Arc<Server> {
    let t = f.target("direct", script, "d", extra);
    let o = t.server_override.unwrap();
    Server::start(
        ServerSpec {
            root: f.dir.path().to_path_buf(),
            program: PathBuf::from(o.command),
            args: o.args,
            env: vec![],
            initialization_options: None,
        },
        None,
    )
    .await
    .expect("fake_lsp starts")
}

async fn edit(server: &std::sync::Arc<Server>, f: &Fixture, name: &str, text: &str) -> Value {
    let path = f.dir.path().join(name);
    std::fs::write(&path, text).unwrap();
    wait_after_write(server, f.dir.path(), &path, text.as_bytes(), "rust")
        .await
        .to_value()
}

#[tokio::test]
async fn a_push_server_reports_the_error_then_clean() {
    let f = Fixture::new();
    let server = start_direct(&f, "diagnostics", &[]).await;
    let broken = edit(&server, &f, "a.rs", "fn a() {}\nlet x = ERROR;\n").await;
    assert_eq!(broken["status"], "current", "{broken}");
    assert_eq!(broken["errors"], 1);
    assert_eq!(broken["items"][0]["path"], "a.rs");
    assert_eq!(broken["items"][0]["line"], 2);
    assert_eq!(broken["items"][0]["severity"], "error");

    let fixed = edit(&server, &f, "a.rs", "fn a() {}\nlet x = 1;\n").await;
    assert_eq!(fixed["status"], "current", "{fixed}");
    assert_eq!(fixed["errors"], 0);
    assert_eq!(fixed["items"].as_array().unwrap().len(), 0);
    server.stop().await;
}

#[tokio::test]
async fn a_pull_server_answers_and_covers_another_open_document() {
    let f = Fixture::new();
    let server = start_direct(&f, "pull", &[]).await;
    let other = f.dir.path().join("b.rs");
    std::fs::write(&other, "let y = ERROR;\n").unwrap();
    server.sync_document(&other, b"let y = ERROR;\n", "rust").await.unwrap();

    let result = edit(&server, &f, "a.rs", "fn a() {}\n").await;
    assert_eq!(result["status"], "current", "{result}");
    assert_eq!(result["errors"], 0);
    assert_eq!(result["other_files"][0]["path"], "b.rs");
    assert_eq!(result["other_files"][0]["errors"], 1);
    server.stop().await;
}

#[tokio::test]
async fn a_slow_server_is_pending_by_three_seconds() {
    let f = Fixture::new();
    let server = start_direct(&f, "slow", &[]).await;
    let began = Instant::now();
    let result = edit(&server, &f, "a.rs", "let x = ERROR;\n").await;
    assert!(began.elapsed() < Duration::from_secs(4), "{:?}", began.elapsed());
    assert_eq!(result["status"], "pending");
    assert_eq!(result["items"].as_array().unwrap().len(), 0);
    server.stop().await;
}

#[tokio::test]
async fn work_in_progress_is_pending_even_when_the_pull_is_clean() {
    let f = Fixture::new();
    let server = start_direct(&f, "pull,progress", &["--progress-ms", "6000"]).await;
    let result = edit(&server, &f, "a.rs", "fn a() {}\n").await;
    assert_eq!(result["status"], "pending", "{result}");
    assert_eq!(result["errors"], 0);
    server.stop().await;
}

#[tokio::test]
async fn an_answer_from_before_the_edit_is_never_current() {
    let f = Fixture::new();
    let server = start_direct(&f, "once", &[]).await;
    let first = edit(&server, &f, "a.rs", "let x = ERROR;\n").await;
    assert_eq!(first["status"], "current");
    assert_eq!(first["errors"], 1);

    // The server publishes nothing for the second version. The old error must not come back as
    // current, and the file must not be called clean either.
    let second = edit(&server, &f, "a.rs", "let x = 1;\n").await;
    assert_eq!(second["status"], "pending", "{second}");
    assert_eq!(second["errors"], 0);
    assert_eq!(second["items"].as_array().unwrap().len(), 0);
    server.stop().await;
}

#[tokio::test]
async fn identical_content_is_not_sent_twice() {
    let f = Fixture::new();
    let server = start_direct(&f, "diagnostics", &[]).await;
    let path = f.dir.path().join("a.rs");
    server.sync_document(&path, b"fn a() {}\n", "rust").await.unwrap();
    server.sync_document(&path, b"fn a() {}\n", "rust").await.unwrap();
    server.sync_document(&path, b"fn b() {}\n", "rust").await.unwrap();
    let too_big = vec![b'a'; 2 * 1024 * 1024 + 1];
    assert!(server.sync_document(&path, &too_big, "rust").await.is_err());
    assert!(server.sync_document(&path, &[0xff, 0xfe], "rust").await.is_err());
    server.stop().await;

    let methods: Vec<String> = log_lines(&f.log("d"))
        .iter()
        .filter_map(|m| m["method"].as_str().map(str::to_owned))
        .filter(|m| m.starts_with("textDocument/"))
        .collect();
    assert_eq!(methods, vec!["textDocument/didOpen", "textDocument/didChange"]);
}

#[tokio::test]
async fn at_most_256_documents_stay_open() {
    let f = Fixture::new();
    let server = start_direct(&f, "pull", &[]).await;
    for i in 0..260 {
        let path = f.dir.path().join(format!("f{i}.rs"));
        server.sync_document(&path, b"x", "rust").await.unwrap();
    }
    assert_eq!(server.open_documents().await.len(), 256);
    assert!(!server.is_open(&f.dir.path().join("f0.rs")).await);
    assert!(server.is_open(&f.dir.path().join("f259.rs")).await);
    server.stop().await;
}

// ---------------------------------------------------------------- Story 4: sharing and bounds

async fn ready_server(host: &LspHost, t: &Target) -> std::sync::Arc<Server> {
    host.ensure(t);
    let began = Instant::now();
    loop {
        if let Availability::Ready(server) = host.ensure(t) {
            return server;
        }
        assert!(began.elapsed() < Duration::from_secs(15), "never ready: {:?}", host.status(&t.project_id));
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn chats_share_a_server_and_a_worktree_has_its_own() {
    let f = Fixture::new();
    let host = LspHost::new(6);
    let main = f.target("p1", "diagnostics", "a", &[]);
    let first = ready_server(&host, &main).await;
    // A second chat, or a subagent piece, asks for the same project, root and language.
    let second = ready_server(&host, &f.target("p1", "diagnostics", "a", &[])).await;
    assert!(std::sync::Arc::ptr_eq(&first, &second));

    let mut worktree = f.target("p1", "diagnostics", "w", &[]);
    worktree.root = f.dir.path().join(".vizier/agent/fix");
    std::fs::create_dir_all(&worktree.root).unwrap();
    let own = ready_server(&host, &worktree).await;
    assert!(!std::sync::Arc::ptr_eq(&first, &own));

    let roots: Vec<String> = host.status("p1").into_iter().map(|s| s.root).collect();
    assert_eq!(roots.len(), 2);
    assert!(roots.contains(&".".to_string()));
    assert!(roots.contains(&".vizier/agent/fix".to_string()));
    host.stop_all().await;
}

#[tokio::test]
async fn the_least_recently_used_server_stops_to_make_room() {
    let f = Fixture::new();
    let host = LspHost::new(1);
    let old = ready_server(&host, &f.target("p1", "diagnostics", "a", &[])).await;
    assert!(matches!(host.ensure(&f.target("p2", "diagnostics", "b", &[])), Availability::Starting { .. }));
    assert!(until(Duration::from_secs(8), || old.exit_info().is_some_and(|e| e.requested)).await);
    assert!(host.status("p1").is_empty());
    assert_eq!(host.capacity().in_use, 1);
    host.stop_all().await;
}

#[tokio::test]
async fn a_server_with_a_question_in_flight_is_not_evicted() {
    let f = Fixture::new();
    let host = LspHost::new(1);
    let busy = ready_server(&host, &f.target("p1", "hang", "a", &[])).await;
    let waiting = {
        let busy = busy.clone();
        tokio::spawn(async move { busy.request("textDocument/hover", json!({})).await })
    };
    assert!(until(Duration::from_secs(3), || busy.in_flight() > 0).await);
    assert!(matches!(host.ensure(&f.target("p2", "diagnostics", "b", &[])), Availability::Busy));
    assert!(busy.exit_info().is_none(), "it was left running");
    waiting.abort();
    host.stop_all().await;
}

#[tokio::test]
async fn an_idle_server_stops() {
    let f = Fixture::new();
    let host = LspHost::with_timing(6, Duration::from_secs(1), Duration::from_millis(200));
    let server = ready_server(&host, &f.target("p1", "diagnostics", "a", &[])).await;
    assert!(until(Duration::from_secs(10), || host.status("p1").is_empty()).await);
    assert!(until(Duration::from_secs(8), || server.exit_info().is_some()).await);
}

// ---------------------------------------------------------------- changes made outside the tools

fn watched_changes(log: &Path) -> Vec<Value> {
    log_lines(log)
        .into_iter()
        .filter(|m| m["method"] == "workspace/didChangeWatchedFiles")
        .flat_map(|m| m["params"]["changes"].as_array().cloned().unwrap_or_default())
        .collect()
}

#[tokio::test]
async fn files_changed_by_a_command_reach_the_server() {
    let f = Fixture::new();
    let root = f.dir.path().to_path_buf();
    std::fs::write(root.join("a.rs"), "one").unwrap();
    std::fs::write(root.join("gone.rs"), "x").unwrap();
    let server = start_direct(&f, "diagnostics", &[]).await;
    assert!(until(Duration::from_secs(10), || server.is_ready()).await);

    std::fs::write(root.join("new.rs"), "n").unwrap();
    std::fs::remove_file(root.join("gone.rs")).unwrap();
    let file = std::fs::OpenOptions::new().write(true).open(root.join("a.rs")).unwrap();
    file.set_modified(std::time::SystemTime::now() + Duration::from_secs(5)).unwrap();

    use vizier_core::projects::lsp::sync::Changes;
    assert_eq!(server.notify_changes(&root).await, Changes::Sent(3));
    assert_eq!(server.notify_changes(&root).await, Changes::Unchanged);
    assert!(until(Duration::from_secs(3), || watched_changes(&f.log("d")).len() == 3).await);
    let mut kinds: Vec<(String, u64)> = watched_changes(&f.log("d"))
        .iter()
        .map(|c| {
            let uri = c["uri"].as_str().unwrap();
            (uri.rsplit('/').next().unwrap().to_string(), c["type"].as_u64().unwrap())
        })
        .collect();
    kinds.sort();
    assert_eq!(kinds, vec![("a.rs".into(), 2), ("gone.rs".into(), 3), ("new.rs".into(), 1)]);
    server.stop().await;
}

#[tokio::test]
async fn too_many_changes_ask_for_a_restart() {
    let f = Fixture::new();
    let root = f.dir.path().to_path_buf();
    let server = start_direct(&f, "diagnostics", &[]).await;
    assert!(until(Duration::from_secs(10), || server.is_ready()).await);
    for i in 0..5_001 {
        std::fs::write(root.join(format!("f{i}.txt")), "x").unwrap();
    }
    use vizier_core::projects::lsp::sync::Changes;
    assert!(matches!(server.notify_changes(&root).await, Changes::TooMany(n) if n >= 5_001));
    server.stop().await;
}

#[tokio::test]
async fn the_host_tells_ready_servers_under_a_folder_in_the_background() {
    let f = Fixture::new();
    let host = LspHost::new(6);
    let t = f.target("p1", "diagnostics", "a", &[]);
    ready_server(&host, &t).await;
    // Give the snapshot a moment: it is taken as the server becomes ready.
    tokio::time::sleep(Duration::from_millis(300)).await;
    std::fs::write(f.dir.path().join("made_by_a_command.rs"), "x").unwrap();
    let began = Instant::now();
    host.notify_changes("p1", f.dir.path());
    assert!(began.elapsed() < Duration::from_millis(100), "it must not wait");
    assert!(until(Duration::from_secs(5), || {
        watched_changes(&f.log("a")).iter().any(|c| c["uri"].as_str().unwrap().ends_with("made_by_a_command.rs"))
    })
    .await);
    host.stop_all().await;
}

// ---------------------------------------------------------------- Story 5: overrides

#[tokio::test]
async fn an_override_wins_and_a_missing_one_has_no_fallback() {
    let f = Fixture::new();
    let host = LspHost::new(6);
    // The rust row's default is rust-analyzer, which may well be installed here. An override that
    // names something else must not fall back to it.
    let mut t = f.target("p1", "x", "a", &[]);
    t.server_override.as_mut().unwrap().command = "my-own-rust-server-xyz".into();
    host.ensure(&t);
    assert!(until(Duration::from_secs(10), || state_of(&host, "p1") == "missing").await);
    let status = &host.status("p1")[0];
    assert!(status.reason.as_deref().unwrap().contains("my-own-rust-server-xyz"), "{status:?}");
    assert!(status.overridden);
    host.stop_all().await;

    // And one that exists is what runs: the fake logs its own handshake.
    let host = LspHost::new(6);
    host.ensure(&f.target("p2", "diagnostics", "b", &[]));
    assert!(until(Duration::from_secs(10), || state_of(&host, "p2") == "ready").await);
    assert!(log_lines(&f.log("b")).iter().any(|m| m["method"] == "initialize"));
    host.stop_all().await;
}

#[tokio::test]
async fn changing_a_language_stops_it_in_every_root_and_leaves_others() {
    let f = Fixture::new();
    let host = LspHost::new(6);
    let rust_a = f.target("p1", "diagnostics", "a", &[]);
    let mut rust_b = f.target("p1", "diagnostics", "b", &[]);
    rust_b.root = f.dir.path().join("sub");
    std::fs::create_dir_all(&rust_b.root).unwrap();
    let mut go = f.target("p1", "diagnostics", "c", &[]);
    go.language = language_by_id("go").unwrap();
    for t in [&rust_a, &rust_b, &go] {
        ready_server(&host, t).await;
    }
    assert_eq!(host.status("p1").len(), 3);
    assert_eq!(host.stop_language("p1", "rust").await, 2);
    let left = host.status("p1");
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].language, "go");
    host.stop_all().await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_binary_inside_the_project_folder_is_never_run_by_name() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let marker = f.logs.path().join("ran-from-the-project");
    for name in ["rust-analyzer", "clangd"] {
        let script = f.dir.path().join(name);
        std::fs::write(&script, format!("#!/bin/sh\ntouch {}\n", marker.display())).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let host = LspHost::new(6);
    let mut t = f.target("p1", "x", "a", &[]);
    t.server_override = None; // the table's own command, looked up by name
    host.ensure(&t);
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(!marker.exists(), "a file in the project folder was run");
    host.stop_all().await;
}
