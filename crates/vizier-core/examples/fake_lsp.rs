//! A scriptable stdio language server for the engine's tests and `specs/054-project-lsp/quickstart.md`.
//!
//! `fake_lsp [--script a,b,...] [--log PATH] [--progress-ms N]`. Modes:
//! `diagnostics` (push an error when a document contains `ERROR`), `once` (push for version 1 only), `pull` (pull diagnostics, with
//! cross-file support), `slow` (diagnostics 5 s late), `progress` (a work-done token open for
//! `--progress-ms`, default 3000), `answers` (definition, references, hover, symbols), `hang` (never
//! answers anything after `initialize`), `slow-start` (3 s before answering `initialize`), `crash`
//! (exits 1 right after `initialized`), `dump-env` (logs its environment), `apply-edit` (asks for a
//! workspace edit and logs the answer), `utf16` / `utf8` (position encoding), `no-init-exit` (prints
//! a "not installed" line and exits before answering).
//!
//! Everything it receives is appended to `--log` as one JSON object per line.

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

type Out = Arc<Mutex<std::io::Stdout>>;

fn send(out: &Out, v: &Value) {
    let body = serde_json::to_vec(v).unwrap();
    let mut o = out.lock().unwrap();
    let _ = write!(o, "Content-Length: {}\r\n\r\n", body.len());
    let _ = o.write_all(&body);
    let _ = o.flush();
}

fn read_frame(r: &mut impl BufRead) -> Option<Value> {
    let mut len = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let t = line.trim();
        if t.is_empty() {
            if len.is_some() {
                break;
            }
            continue;
        }
        if let Some(v) = t.to_ascii_lowercase().strip_prefix("content-length:") {
            len = v.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0u8; len?];
    r.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

fn diagnostics_for(text: &str) -> Vec<Value> {
    let mut items = vec![];
    for (line, content) in text.lines().enumerate() {
        if let Some(col) = content.find("ERROR") {
            items.push(json!({
                "range": {"start": {"line": line, "character": col}, "end": {"line": line, "character": col + 5}},
                "severity": 1, "code": "E0001", "source": "fake", "message": "found ERROR"
            }));
        }
        if let Some(col) = content.find("WARN") {
            items.push(json!({
                "range": {"start": {"line": line, "character": col}, "end": {"line": line, "character": col + 4}},
                "severity": 2, "source": "fake", "message": "found WARN"
            }));
        }
    }
    items
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut modes: Vec<String> = vec![];
    let mut log_path: Option<String> = None;
    let mut progress_ms = 3000u64;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--script" => {
                i += 1;
                modes = args[i].split(',').map(str::to_owned).collect();
            }
            "--log" => {
                i += 1;
                log_path = Some(args[i].clone());
            }
            "--progress-ms" => {
                i += 1;
                progress_ms = args[i].parse().unwrap_or(3000);
            }
            _ => {}
        }
        i += 1;
    }
    let has = |m: &str| modes.iter().any(|x| x == m);
    let log = Arc::new(Mutex::new(
        log_path.map(|p| OpenOptions::new().create(true).append(true).open(p).unwrap()),
    ));
    let write_log = {
        let log = log.clone();
        move |v: &Value| {
            if let Some(f) = log.lock().unwrap().as_mut() {
                let _ = writeln!(f, "{v}");
            }
        }
    };

    if has("no-init-exit") {
        eprintln!("error: fake-lsp is not installed for this toolchain");
        std::process::exit(1);
    }
    if has("dump-env") {
        let env: HashMap<String, String> = std::env::vars().collect();
        write_log(&json!({"env": env}));
    }

    let out: Out = Arc::new(Mutex::new(std::io::stdout()));
    let mut reader = BufReader::new(std::io::stdin());
    let mut docs: HashMap<String, (String, i64)> = HashMap::new();

    while let Some(msg) = read_frame(&mut reader) {
        write_log(&msg);
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let id = msg.get("id").cloned();
        let params = msg.get("params").cloned().unwrap_or(Value::Null);

        if method == "initialize" {
            if has("slow-start") {
                std::thread::sleep(Duration::from_secs(3));
            }
            let mut caps = json!({
                "textDocumentSync": {"openClose": true, "change": 1, "save": {}},
                "definitionProvider": true, "referencesProvider": true, "hoverProvider": true,
                "documentSymbolProvider": true, "workspaceSymbolProvider": true
            });
            if has("pull") {
                caps["diagnosticProvider"] =
                    json!({"interFileDependencies": true, "workspaceDiagnostics": false});
            }
            if has("utf8") {
                caps["positionEncoding"] = json!("utf-8");
            }
            send(&out, &json!({"jsonrpc":"2.0","id":id,"result":{"capabilities":caps}}));
            continue;
        }
        if method == "initialized" {
            if has("crash") {
                std::process::exit(1);
            }
            if has("progress") {
                send(&out, &json!({"jsonrpc":"2.0","id":"p1","method":"window/workDoneProgress/create","params":{"token":"idx"}}));
                send(&out, &json!({"jsonrpc":"2.0","method":"$/progress","params":{"token":"idx","value":{"kind":"begin","title":"Indexing","percentage":0}}}));
                let out2 = out.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(progress_ms / 2));
                    send(&out2, &json!({"jsonrpc":"2.0","method":"$/progress","params":{"token":"idx","value":{"kind":"report","percentage":50}}}));
                    std::thread::sleep(Duration::from_millis(progress_ms / 2));
                    send(&out2, &json!({"jsonrpc":"2.0","method":"$/progress","params":{"token":"idx","value":{"kind":"end"}}}));
                });
            }
            if has("apply-edit") {
                send(&out, &json!({"jsonrpc":"2.0","id":"e1","method":"workspace/applyEdit","params":{"edit":{"changes":{}}}}));
            }
            continue;
        }
        if has("hang") && id.is_some() && method != "shutdown" {
            continue;
        }
        let delay = if has("slow") { Duration::from_secs(5) } else { Duration::ZERO };
        match method {
            "textDocument/didOpen" => {
                let td = &params["textDocument"];
                let uri = td["uri"].as_str().unwrap_or("").to_owned();
                let text = td["text"].as_str().unwrap_or("").to_owned();
                let version = td["version"].as_i64().unwrap_or(0);
                docs.insert(uri.clone(), (text.clone(), version));
                push(&out, &modes, &uri, &text, version, delay);
            }
            "textDocument/didChange" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("").to_owned();
                let version = params["textDocument"]["version"].as_i64().unwrap_or(0);
                let text = params["contentChanges"][0]["text"].as_str().unwrap_or("").to_owned();
                docs.insert(uri.clone(), (text.clone(), version));
                push(&out, &modes, &uri, &text, version, delay);
            }
            "textDocument/didClose" => {
                if let Some(uri) = params["textDocument"]["uri"].as_str() {
                    docs.remove(uri);
                }
            }
            "textDocument/diagnostic" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
                let items = docs.get(uri).map(|d| diagnostics_for(&d.0)).unwrap_or_default();
                let mut related = serde_json::Map::new();
                for (u, (t, _)) in &docs {
                    if u != uri {
                        related.insert(u.clone(), json!({"kind":"full","items":diagnostics_for(t)}));
                    }
                }
                let reply = json!({"jsonrpc":"2.0","id":id,"result":{"kind":"full","items":items,"relatedDocuments":related}});
                let out2 = out.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(delay);
                    send(&out2, &reply);
                });
            }
            "textDocument/definition" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
                let r = |l: u64, c: u64| json!({"start":{"line":l,"character":c},"end":{"line":l,"character":c+3}});
                send(&out, &json!({"jsonrpc":"2.0","id":id,"result":[
                    {"uri":uri,"range":r(2,4)},
                    {"uri":"file:///fake-outside/option.rs","range":r(570,0)}
                ]}));
            }
            "textDocument/references" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
                let locs: Vec<Value> = (0..60u64)
                    .map(|l| json!({"uri":uri,"range":{"start":{"line":l,"character":0},"end":{"line":l,"character":3}}}))
                    .collect();
                send(&out, &json!({"jsonrpc":"2.0","id":id,"result":locs}));
            }
            "textDocument/hover" => {
                send(&out, &json!({"jsonrpc":"2.0","id":id,"result":{"contents":{"kind":"markdown","value":"```rust\nfn resolve()\n```\nResolves a path."}}}));
            }
            "textDocument/documentSymbol" => {
                send(&out, &json!({"jsonrpc":"2.0","id":id,"result":[
                    {"name":"resolve","kind":12,"range":{"start":{"line":11,"character":0},"end":{"line":14,"character":1}},"selectionRange":{"start":{"line":11,"character":3},"end":{"line":11,"character":10}}}
                ]}));
            }
            "workspace/symbol" => {
                send(&out, &json!({"jsonrpc":"2.0","id":id,"result":[
                    {"name":"Resolver","kind":5,"containerName":"paths","location":{"uri":"file:///fake-outside/x.rs","range":{"start":{"line":3,"character":0},"end":{"line":3,"character":8}}}}
                ]}));
            }
            "shutdown" => send(&out, &json!({"jsonrpc":"2.0","id":id,"result":null})),
            "exit" => std::process::exit(0),
            _ => {
                if let Some(id) = id
                    && msg.get("method").is_some()
                {
                    send(&out, &json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"method not found"}}));
                }
            }
        }
    }
}

fn push(out: &Out, modes: &[String], uri: &str, text: &str, version: i64, delay: Duration) {
    // `once` publishes for the first version only, so a later edit gets no answer.
    if version > 1 && modes.iter().any(|m| m == "once") {
        return;
    }
    if !modes.iter().any(|m| m == "diagnostics" || m == "slow" || m == "once") || modes.iter().any(|m| m == "pull") {
        return;
    }
    let msg = json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{
        "uri":uri,"version":version,"diagnostics":diagnostics_for(text)}});
    let out = out.clone();
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        send(&out, &msg);
    });
}
