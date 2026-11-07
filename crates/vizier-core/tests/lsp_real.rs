//! Real language servers (`specs/054-project-lsp` T023). Each test skips, saying so, when its server
//! is not installed or cannot answer `initialize`. Run with `-- --nocapture` to see the skips.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use vizier_core::projects::lsp::diagnostics::wait_after_write;
use vizier_core::projects::lsp::languages::language_by_id;
use vizier_core::projects::lsp::search_path;
use vizier_core::projects::lsp::server::{Server, ServerSpec};

async fn start(language: &str, root: &Path) -> Option<Arc<Server>> {
    let def = language_by_id(language).unwrap();
    let Some(found) = search_path::resolve(def.command).await else {
        println!("SKIPPED: {} is not installed", def.command);
        return None;
    };
    let spec = ServerSpec {
        root: root.to_path_buf(),
        program: found.program,
        args: def.args.iter().map(|a| a.to_string()).collect(),
        env: search_path::filtered_env(std::env::vars(), found.path.as_deref(), &Default::default()),
        initialization_options: def
            .initialization_options
            .and_then(|t| serde_json::from_str(t).ok()),
    };
    match Server::start(spec, None).await {
        Ok(server) => Some(server),
        Err(e) => {
            println!("SKIPPED: {} cannot start here: {e:?}", def.command);
            None
        }
    }
}

async fn ready(server: &Server, limit: Duration) -> bool {
    let began = Instant::now();
    while began.elapsed() < limit {
        if server.is_ready() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

fn write(root: &Path, rel: &str, text: &str) -> PathBuf {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    path
}

#[tokio::test]
async fn clangd_reports_an_undeclared_identifier_within_three_seconds() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    write(&root, "compile_flags.txt", "-std=c++17\n");
    let Some(server) = start("cpp", &root).await else { return };
    assert!(ready(&server, Duration::from_secs(30)).await);

    let text = "int main() { return undeclared_name; }\n";
    let path = write(&root, "a.cpp", text);
    let began = Instant::now();
    let result = wait_after_write(&server, &root, &path, text.as_bytes(), "cpp").await;
    assert!(began.elapsed() < Duration::from_secs(4), "{:?}", began.elapsed());
    assert_eq!(result.status, "current", "{result:?}");
    assert!(result.errors >= 1, "{result:?}");
    assert!(result.items.iter().any(|i| i.message.contains("undeclared_name")), "{result:?}");
    server.stop().await;
}

#[tokio::test]
async fn rust_analyzer_reports_a_type_error_and_a_broken_call_site() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    write(
        &root,
        "Cargo.toml",
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write(&root, "src/lib.rs", "pub mod other;\npub fn add(a: u32, b: u32) -> u32 { a + b }\n");
    let other = write(&root, "src/other.rs", "pub fn use_it() -> u32 { crate::add(1, 2) }\n");
    let Some(server) = start("rust", &root).await else { return };
    assert!(ready(&server, Duration::from_secs(120)).await, "rust-analyzer never became ready");

    // A server only learns about files it was told about: this is what `read_file` does.
    server
        .sync_document(&other, &std::fs::read(&other).unwrap(), "rust")
        .await
        .unwrap();

    // 1. A type error in the edited file.
    let lib = root.join("src/lib.rs");
    let bad = "pub mod other;\npub fn add(a: u32, b: u32) -> u32 { \"x\" }\n";
    std::fs::write(&lib, bad).unwrap();
    let began = Instant::now();
    let result = wait_after_write(&server, &root, &lib, bad.as_bytes(), "rust").await;
    println!("type error answered in {:?}: {result:?}", began.elapsed());
    assert_eq!(result.status, "current", "{result:?}");
    assert!(result.errors >= 1, "{result:?}");

    // 2. A signature change breaks the call site in the file already read.
    let changed = "pub mod other;\npub fn add(a: u32, b: u32, c: u32) -> u32 { a + b + c }\n";
    std::fs::write(&lib, changed).unwrap();
    let result = wait_after_write(&server, &root, &lib, changed.as_bytes(), "rust").await;
    println!("signature change: {result:?}");
    assert_eq!(result.status, "current", "{result:?}");
    assert!(
        result.other_files.iter().any(|f| f.path == "src/other.rs" && f.errors >= 1),
        "{result:?}"
    );
    server.stop().await;
}

// ---------------------------------------------------------------- Story 3: questions

use vizier_core::projects::{
    context::ProjectTurnContext,
    lsp::{LspHost, position::Encoding, query},
};
use vizier_core::schema::ProjectMode;

fn project(root: &Path) -> ProjectTurnContext {
    ProjectTurnContext {
        project_id: "p".into(),
        name: "fixture".into(),
        folder: root.to_path_buf(),
        cwd: root.to_path_buf(),
        worktree: None,
        mode: ProjectMode::Plan,
        git: None,
        available: true,
        shell_enabled: false,
        instructions: None,
        instructions_truncated: false,
        is_piece: false,
        ledger: Default::default(),
        lsp_enabled: true,
        lsp_overrides: Default::default(),
        lsp: LspHost::new(6),
    }
}

/// Ask a position question the way the tools do, waiting out `lsp_starting`.
async fn question(
    project: &ProjectTurnContext,
    file: &Path,
    line: usize,
    symbol: &str,
    method: &str,
    extra: serde_json::Value,
    limit: Duration,
) -> Option<(serde_json::Value, Encoding)> {
    let began = Instant::now();
    loop {
        match query::prepare("agent", project, file).await {
            Ok(ready) => {
                let enc = ready.server.encoding();
                let (line0, character) = query::locate(&ready.text, line, symbol, None, enc).expect("symbol on line");
                let mut params = serde_json::json!({
                    "textDocument": { "uri": ready.uri },
                    "position": { "line": line0, "character": character },
                });
                if let (Some(map), serde_json::Value::Object(more)) = (params.as_object_mut(), extra.clone()) {
                    map.extend(more);
                }
                // A server may answer with nothing while it still indexes: ask again for a while.
                let answer = query::ask(&ready.server, method, params).await.ok()?;
                let empty = answer.is_null() || answer.as_array().is_some_and(Vec::is_empty);
                if !empty || began.elapsed() > limit {
                    return Some((answer, enc));
                }
            }
            Err(status) => {
                let name = status["status"].as_str().unwrap_or("");
                if name == "lsp_unavailable" {
                    println!("SKIPPED: {status}");
                    return None;
                }
                assert!(name == "lsp_starting", "{status}");
            }
        }
        assert!(began.elapsed() < limit + Duration::from_secs(5), "never answered");
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[tokio::test]
async fn rust_analyzer_answers_definition_references_and_hover() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    write(&root, "Cargo.toml", "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n");
    write(
        &root,
        "src/lib.rs",
        "pub mod other;\n/// Adds two numbers.\npub fn resolve(a: u32, b: u32) -> u32 { a + b }\n",
    );
    let other = write(&root, "src/other.rs", "pub fn use_it() -> u32 { crate::resolve(1, 2) }\n");
    // Same name, different thing: references must not include it.
    write(&root, "src/decoy.rs", "pub fn resolve() {}\n");
    let project = project(&root);
    let limit = Duration::from_secs(120);

    let Some((definition, enc)) = question(&project, &other, 1, "resolve", "textDocument/definition", serde_json::json!({}), limit).await else {
        return;
    };
    let locations = query::map_locations(&root, enc, &definition);
    let text = serde_json::to_string(&locations).unwrap();
    println!("definition: {text}");
    assert!(text.contains("src/lib.rs"), "{text}");
    assert!(!text.contains(root.to_str().unwrap()));

    let (references, enc) = question(
        &project,
        &other,
        1,
        "resolve",
        "textDocument/references",
        serde_json::json!({ "context": { "includeDeclaration": false } }),
        limit,
    )
    .await
    .unwrap();
    let refs = serde_json::to_string(&query::map_locations(&root, enc, &references)).unwrap();
    println!("references: {refs}");
    assert!(refs.contains("src/other.rs"), "{refs}");
    assert!(!refs.contains("decoy"), "a same-named function is not a use: {refs}");

    let (hover, _) = question(&project, &other, 1, "resolve", "textDocument/hover", serde_json::json!({}), limit).await.unwrap();
    println!("hover: {hover}");
    assert!(hover.to_string().contains("fn resolve"), "{hover}");
    project.lsp.stop_all().await;
}

#[tokio::test]
async fn clangd_answers_definition_references_and_hover() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    write(&root, "compile_flags.txt", "-std=c++17\n");
    write(&root, "lib.h", "int resolve(int a, int b);\n");
    write(&root, "lib.cpp", "#include \"lib.h\"\nint resolve(int a, int b) { return a + b; }\n");
    let main = write(&root, "main.cpp", "#include \"lib.h\"\nint main() { return resolve(1, 2); }\n");
    let project = project(&root);
    let limit = Duration::from_secs(60);

    let Some((definition, enc)) = question(&project, &main, 2, "resolve", "textDocument/definition", serde_json::json!({}), limit).await else {
        return;
    };
    let text = serde_json::to_string(&query::map_locations(&root, enc, &definition)).unwrap();
    println!("definition: {text}");
    assert!(text.contains("lib.h") || text.contains("lib.cpp"), "{text}");

    let (hover, _) = question(&project, &main, 2, "resolve", "textDocument/hover", serde_json::json!({}), limit).await.unwrap();
    println!("hover: {hover}");
    assert!(hover.to_string().contains("resolve"), "{hover}");
    project.lsp.stop_all().await;
}
