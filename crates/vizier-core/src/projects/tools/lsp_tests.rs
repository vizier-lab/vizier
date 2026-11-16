//! The five `lsp_*` tools against the scriptable `fake_lsp` example (`specs/054-project-lsp` US3).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::Value;

use super::{
    lsp_common::PositionInput, lsp_definition::LspDefinition, lsp_diagnostics::LspDiagnostics,
    lsp_hover::LspHover, lsp_references::{LspReferences, LspReferencesInput},
    lsp_symbols::{LspSymbols, LspSymbolsInput}, offered,
};
use crate::{
    agents::tools::{ToolContext, VizierTool},
    projects::{
        context::ProjectTurnContext,
        lsp::{Availability, LspHost, target_for},
    },
    schema::{ProjectMode, ServerOverride, VizierChannelId, VizierSession},
};

fn fake_lsp() -> PathBuf {
    crate::tests_support::fake_lsp_path()
}

struct World {
    dir: tempfile::TempDir,
    ctx: ToolContext,
    log: PathBuf,
}

fn world(script: &str, extra: &[&str], mode: ProjectMode, lsp_enabled: bool) -> World {
    let dir = tempfile::tempdir().unwrap();
    let folder = std::fs::canonicalize(dir.path()).unwrap();
    let log = folder.join("server.log");
    let mut args = vec!["--script".to_string(), script.to_string(), "--log".into(), log.to_string_lossy().into_owned()];
    args.extend(extra.iter().map(|s| s.to_string()));
    let overrides = [(
        "rust".to_string(),
        ServerOverride { command: fake_lsp().to_string_lossy().into_owned(), args, env: Default::default() },
    )]
    .into();
    let project = ProjectTurnContext {
        project_id: "p".into(),
        name: "alpha".into(),
        folder: folder.clone(),
        cwd: folder,
        worktree: None,
        mode,
        git: None,
        available: true,
        shell_enabled: true,
        instructions: None,
        instructions_truncated: false,
        is_piece: false,
        ledger: Default::default(),
        lsp_enabled,
        lsp_overrides: Arc::new(overrides),
        lsp: LspHost::new(6),
    };
    let ctx = ToolContext {
        session: VizierSession(
            "dp-a".into(),
            VizierChannelId::Project("alice".into(), "p".into()),
            Some("t1".into()),
        ),
        pending_attachments: Default::default(),
        hooks: None,
        background_depth: 0,
        subagent_depth: 0,
        loaded_images: Default::default(),
        deadline: None,
        client: None,
        questions: crate::questions::QuestionsHandle::new(Default::default()),
        interactive: false,
        secret_redactor: Arc::new(crate::secrets::SecretRedactor::default()),
        dream_sources: vec![],
        project: Some(Arc::new(project)),
        detached: false,
        parent_detached_id: None,
        custom: Default::default(),
    };
    World { dir, ctx, log }
}

impl World {
    fn write(&self, rel: &str, text: &str) {
        let path = self.dir.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn project(&self) -> &ProjectTurnContext {
        self.ctx.project.as_deref().unwrap()
    }

    /// Wait until the server for `src/a.rs` is ready.
    async fn ready(&self) {
        let file = std::fs::canonicalize(self.dir.path()).unwrap().join("src/a.rs");
        let target = target_for("dp-a", self.project(), &file).expect("a target");
        let began = Instant::now();
        loop {
            let availability = self.project().lsp.ensure(&target);
            if matches!(availability, Availability::Ready(_)) {
                return;
            }
            // Say why, so a failure on a machine nobody can reproduce on is still readable.
            assert!(
                began.elapsed() < Duration::from_secs(20),
                "the server never became ready: {availability:?} (fixture {} exists: {})",
                fake_lsp().display(),
                fake_lsp().exists(),
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    async fn stop(&self) {
        self.project().lsp.stop_all().await;
    }
}

fn at(path: &str, line: usize, symbol: &str) -> PositionInput {
    PositionInput { path: path.into(), line, symbol: symbol.into(), occurrence: None }
}

fn lines(n: usize) -> String {
    (1..=n).map(|i| format!("line {i} resolve\n")).collect()
}

#[tokio::test]
async fn a_question_to_a_starting_server_answers_at_once() {
    let w = world("answers,progress", &["--progress-ms", "3000"], ProjectMode::Plan, true);
    w.write("src/a.rs", &lines(20));
    let began = Instant::now();
    let out = LspDefinition.call(at("src/a.rs", 1, "resolve"), &w.ctx).await.unwrap();
    assert!(began.elapsed() < Duration::from_secs(1), "{:?}", began.elapsed());
    assert_eq!(out["status"], "lsp_starting", "{out}");
    w.stop().await;
}

#[tokio::test]
async fn definition_maps_locations_and_never_shows_an_absolute_path() {
    let w = world("answers", &[], ProjectMode::Plan, true);
    w.write("src/a.rs", &lines(20));
    w.ready().await;
    let out = LspDefinition.call(at("src/a.rs", 1, "resolve"), &w.ctx).await.unwrap();
    assert_eq!(out["status"], "ok", "{out}");
    assert_eq!(out["locations"][0]["path"], "src/a.rs");
    assert_eq!(out["locations"][0]["line"], 3);
    assert_eq!(out["locations"][1]["outside_project"], true);
    assert_eq!(out["locations"][1]["file"], "option.rs");
    let text = out.to_string();
    assert!(!text.contains(w.dir.path().to_str().unwrap()), "{text}");
    assert!(!text.contains("fake-outside"), "{text}");
    w.stop().await;
}

#[tokio::test]
async fn sixty_references_are_cut_to_fifty_with_a_hint() {
    let w = world("answers", &[], ProjectMode::Plan, true);
    w.write("src/a.rs", &lines(70));
    w.ready().await;
    let input = LspReferencesInput { position: at("src/a.rs", 1, "resolve"), include_declaration: None };
    let out = LspReferences.call(input, &w.ctx).await.unwrap();
    assert_eq!(out["status"], "ok", "{out}");
    assert_eq!(out["locations"].as_array().unwrap().len(), 50);
    assert_eq!(out["truncated"], true);
    assert!(out["hint"].as_str().unwrap().contains("narrow") || out["hint"].as_str().unwrap().contains("specific"));
    w.stop().await;
}

#[tokio::test]
async fn hover_splits_signature_and_docs() {
    let w = world("answers", &[], ProjectMode::Plan, true);
    w.write("src/a.rs", &lines(5));
    w.ready().await;
    let out = LspHover.call(at("src/a.rs", 1, "resolve"), &w.ctx).await.unwrap();
    assert_eq!(out["signature"], "fn resolve()");
    assert_eq!(out["docs"], "Resolves a path.");
    w.stop().await;
}

#[tokio::test]
async fn symbols_of_a_file_and_a_search() {
    let w = world("answers", &[], ProjectMode::Plan, true);
    w.write("src/a.rs", &lines(20));
    w.ready().await;
    let out = LspSymbols.call(LspSymbolsInput { path: "src/a.rs".into(), query: None }, &w.ctx).await.unwrap();
    assert_eq!(out["status"], "ok", "{out}");
    assert_eq!(out["symbols"][0]["name"], "resolve");
    assert_eq!(out["symbols"][0]["kind"], "function");
    assert_eq!(out["symbols"][0]["path"], "src/a.rs");
    assert_eq!(out["symbols"][0]["line"], 12);

    // A search: the fake answers with a symbol outside the project, which is left out.
    let out = LspSymbols
        .call(LspSymbolsInput { path: "src".into(), query: Some("Resolver".into()) }, &w.ctx)
        .await
        .unwrap();
    assert_eq!(out["status"], "ok", "{out}");
    assert_eq!(out["symbols"].as_array().unwrap().len(), 0);
    assert!(!out.to_string().contains("fake-outside"));

    // A folder without `query` has no meaning.
    let err = LspSymbols.call(LspSymbolsInput { path: "src".into(), query: None }, &w.ctx).await;
    assert!(err.is_err());
    w.stop().await;
}

#[tokio::test]
async fn a_symbol_not_on_the_line_says_what_is() {
    let w = world("answers", &[], ProjectMode::Plan, true);
    w.write("src/a.rs", "fn only() {}\n");
    w.ready().await;
    let out = LspDefinition.call(at("src/a.rs", 1, "missing"), &w.ctx).await.unwrap();
    assert_eq!(out["status"], "symbol_not_on_line");
    assert_eq!(out["line_text"], "fn only() {}");
    w.stop().await;
}

#[tokio::test]
async fn a_file_no_language_owns_is_no_language() {
    let w = world("answers", &[], ProjectMode::Plan, true);
    w.write("notes.txt", "hello\n");
    let out = LspDefinition.call(at("notes.txt", 1, "hello"), &w.ctx).await.unwrap();
    assert_eq!(out["status"], "no_language");
}

#[tokio::test]
async fn a_switch_turned_off_mid_turn_reads_lsp_disabled() {
    let w = world("answers", &[], ProjectMode::Plan, false);
    w.write("src/a.rs", "fn a() {}\n");
    let out = LspHover.call(at("src/a.rs", 1, "a"), &w.ctx).await.unwrap();
    assert_eq!(out["status"], "lsp_disabled");

    // The turn began on, then the host closed the project.
    let w = world("answers", &[], ProjectMode::Plan, true);
    w.write("src/a.rs", "fn a() {}\n");
    w.project().lsp.close_project("p").await;
    let out = LspHover.call(at("src/a.rs", 1, "a"), &w.ctx).await.unwrap();
    assert_eq!(out["status"], "lsp_disabled");
}

#[tokio::test]
async fn a_missing_server_is_unavailable_with_its_install_hint() {
    let mut w = world("answers", &[], ProjectMode::Plan, true);
    {
        let project = Arc::make_mut(w.ctx.project.as_mut().unwrap());
        let mut overrides = (*project.lsp_overrides).clone();
        overrides.get_mut("rust").unwrap().command = "definitely-not-installed-xyz".into();
        project.lsp_overrides = Arc::new(overrides);
    }
    w.write("src/a.rs", "fn a() {}\n");
    let began = Instant::now();
    let mut out = Value::Null;
    while began.elapsed() < Duration::from_secs(10) {
        out = LspHover.call(at("src/a.rs", 1, "a"), &w.ctx).await.unwrap();
        if out["status"] == "lsp_unavailable" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(out["status"], "lsp_unavailable", "{out}");
    assert_eq!(out["server"], "definitely-not-installed-xyz");
    assert!(out["install_hint"].as_str().unwrap().contains("rustup"));
}

#[tokio::test]
async fn positions_follow_the_negotiated_encoding() {
    for (script, expected) in [("answers", 3), ("answers,utf8", 5)] {
        let w = world(script, &[], ProjectMode::Plan, true);
        w.write("src/a.rs", "a\u{1F600}b\n");
        w.ready().await;
        let out = LspDefinition.call(at("src/a.rs", 1, "b"), &w.ctx).await.unwrap();
        assert_eq!(out["status"], "ok", "{out}");
        let log = std::fs::read_to_string(&w.log).unwrap();
        let request = log
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .find(|m| m["method"] == "textDocument/definition")
            .expect("a definition request");
        assert_eq!(request["params"]["position"]["character"], expected, "{script}");
        w.stop().await;
    }
}

#[tokio::test]
async fn diagnostics_of_a_file_and_of_a_folder() {
    let w = world("diagnostics", &[], ProjectMode::Plan, true);
    w.write("src/a.rs", "let x = ERROR;\n");
    w.ready().await;
    let out = LspDiagnostics
        .call(super::lsp_diagnostics::LspDiagnosticsInput { path: Some("src/a.rs".into()) }, &w.ctx)
        .await
        .unwrap();
    assert_eq!(out["status"], "ok", "{out}");
    assert_eq!(out["freshness"], "current");
    assert_eq!(out["errors"], 1);
    assert_eq!(out["items"][0]["path"], "src/a.rs");

    let folder = LspDiagnostics
        .call(super::lsp_diagnostics::LspDiagnosticsInput { path: Some("src".into()) }, &w.ctx)
        .await
        .unwrap();
    assert_eq!(folder["scope"], "reported");
    assert_eq!(folder["errors"], 1, "{folder}");
    w.stop().await;
}

fn names(project: &ProjectTurnContext) -> Vec<String> {
    offered(project).into_iter().map(|e| e.name).collect()
}

#[test]
fn the_tools_are_offered_in_plan_and_build_and_absent_when_off() {
    const LSP: [&str; 5] = ["lsp_diagnostics", "lsp_definition", "lsp_references", "lsp_hover", "lsp_symbols"];
    for mode in [ProjectMode::Plan, ProjectMode::Build] {
        let on = world("answers", &[], mode, true);
        let offered = names(on.project());
        for name in LSP {
            assert!(offered.contains(&name.to_string()), "{name} in {mode:?}");
        }
        let off = world("answers", &[], mode, false);
        let offered = names(off.project());
        for name in LSP {
            assert!(!offered.contains(&name.to_string()), "{name} offered while off");
        }
    }
}

#[tokio::test]
async fn a_script_cannot_reach_a_tool_that_is_not_offered() {
    let w = world("answers", &[], ProjectMode::Plan, false);
    let err = super::call("lsp_hover", r#"{"path":"a.rs","line":1,"symbol":"a"}"#.into(), &w.ctx)
        .await
        .unwrap_err();
    assert!(err.0.contains("tool_not_offered"), "{}", err.0);
    assert!(err.0.contains("language servers are off"), "{}", err.0);
}

#[tokio::test]
async fn listing_and_searching_files_start_no_server() {
    let w = world("answers", &[], ProjectMode::Build, true);
    w.write("src/a.rs", "fn a() {}\n");
    for (name, args) in [
        ("grep", r#"{"pattern":"fn"}"#),
        ("glob", r#"{"pattern":"**/*.rs"}"#),
        ("list_files", r#"{}"#),
    ] {
        super::call(name, args.into(), &w.ctx).await.unwrap();
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(w.project().lsp.capacity().in_use, 0);
    assert!(w.project().lsp.status("p").is_empty());
}

#[tokio::test]
async fn reading_a_file_starts_its_server_in_the_background() {
    let w = world("answers", &[], ProjectMode::Plan, true);
    w.write("src/a.rs", "fn a() {}\n");
    let began = Instant::now();
    super::call("read_file", r#"{"path":"src/a.rs"}"#.into(), &w.ctx).await.unwrap();
    assert!(began.elapsed() < Duration::from_millis(500), "the read must not wait for the server");
    assert_eq!(w.project().lsp.capacity().in_use, 1);
    w.stop().await;
}

#[tokio::test]
async fn a_config_file_in_the_project_changes_no_server() {
    let w = world("answers", &[], ProjectMode::Plan, true);
    w.write(".vscode/settings.json", r#"{"rust-analyzer.server.path": "/tmp/evil"}"#);
    w.write("rust-analyzer.toml", "[server]\npath = \"/tmp/evil\"\n");
    w.write("src/a.rs", "fn a() {}\n");
    let file = std::fs::canonicalize(w.dir.path()).unwrap().join("src/a.rs");
    let target = target_for("dp-a", w.project(), &file).unwrap();
    // Only a person's saved override (here the fake) decides the command.
    let command = target.server_override.unwrap().command;
    assert!(
        std::path::Path::new(&command).file_stem().is_some_and(|stem| stem == "fake_lsp"),
        "{command}"
    );
}
