# Data Model: Project Language Intelligence

**Feature**: `054-project-lsp`

## 1. `project.lsp_enabled` (stored)

| Field | Type | Notes |
|---|---|---|
| `lsp_enabled` | `INTEGER` / `BIGINT`, `NOT NULL DEFAULT 0` | `Project.lsp_enabled: bool` |

- **SQLite**: `add_column_if_missing` in `init_project_schema` (add a checklist entry to #82).
- **PostgreSQL**: migration `0005_project_lsp.sql`.
- **The column default is 0, so existing projects stay off.** `open_project` writes `1` unless the
  request says otherwise. Reopening a soft-removed project keeps its value.

## 2. `project_lsp_override` (stored)

One row per project and language. It applies to every root of that language.

| Column | Type | Rules |
|---|---|---|
| `project_id` | TEXT, FK `project(id) ON DELETE CASCADE` | Part of the primary key |
| `language` | TEXT | Part of the primary key; must be a `languages.rs` id |
| `command` | TEXT | Non-empty; at most 1,024 bytes; no newlines |
| `args` | TEXT (JSON array of strings) | At most 32 items |
| `env` | TEXT (encrypted envelope of a JSON map) | At most 32 keys; names match `[A-Za-z_][A-Za-z0-9_]*` |
| `updated_by` | TEXT | `user_id` |
| `updated_at` | INTEGER | Epoch ms |

- Written only through `service::project_lsp`, which checks `access::can_manage`. No agent tool writes it.
- `env` AAD: `vizier:secrets:v1 \x1f project_lsp_override \x1f {project_id}/{language} \x1f env`.
- Deleted with the project row, and so with agent deletion.
- Confirm in implementation that `vizier storage migrate` copies the new table.

## 3. `LanguageDef` (built in, not stored)

`crates/vizier-core/src/projects/lsp/languages.rs`:

| Field | Example |
|---|---|
| `id` | `rust` |
| `name` | `Rust` |
| `extensions` | `["rs"]` |
| `lsp_language_id` | `rust` (per extension for TS: `typescript`, `typescriptreact`, `javascript`, `javascriptreact`) |
| `command`, `args` | `rust-analyzer`, `[]` |
| `root` | `RootRule::CargoWorkspace` |
| `install_hint` | `rustup component add rust-analyzer` |

Rows:

| Language | Command | Install hint |
|---|---|---|
| Rust | `rust-analyzer` | `rustup component add rust-analyzer` |
| TypeScript/JavaScript | `typescript-language-server --stdio` | `npm install -g typescript typescript-language-server` |
| Python | `pyright-langserver --stdio` | `npm install -g pyright` |
| Go | `gopls` | `go install golang.org/x/tools/gopls@latest` |
| C/C++ | `clangd` | Install LLVM or the Xcode command line tools |
| Lua | `lua-language-server` | See its project page |
| Java | `jdtls` | `brew install jdtls`, or the Eclipse JDT LS downloads |
| Kotlin | `kotlin-language-server` | `brew install kotlin-language-server` |
| C# | `csharp-ls` | `dotnet tool install --global csharp-ls` |
| Ruby | `ruby-lsp` | `gem install ruby-lsp` |
| PHP | `intelephense --stdio` | `npm install -g intelephense` |
| Swift | `sourcekit-lsp` | Comes with the Swift toolchain or Xcode |
| Dart | `dart language-server` | Comes with the Dart or Flutter SDK |
| Elixir | `elixir-ls` | `brew install elixir-ls` |
| Zig | `zls` | `brew install zls` |
| Bash | `bash-language-server start` | `npm install -g bash-language-server` |

Install hints are shown to people as they are. Checked when `languages.rs` was written: the Homebrew
formulae (jdtls, kotlin-language-server, elixir-ls, zls, lua-language-server), the npm packages and
their binaries (typescript-language-server, pyright's `pyright-langserver`, intelephense,
bash-language-server) and the `ruby-lsp` gem all exist. Only clangd and rust-analyzer have been *run*;
`gopls`, `csharp-ls`, `sourcekit-lsp` and `dart language-server` could not be installed here.

## 4. `ServerInstance` (memory only)

Key: `(project_id, root: PathBuf, language)`.

| Field | Notes |
|---|---|
| `state` | See transitions below |
| `progress` | Latest title and percentage, for "indexing 41%" |
| `failure` | Reason text, from the capped stderr ring or the exit code |
| `restarts` | Timestamps within the last 10 minutes |
| `last_used` | For LRU and the idle stop |
| `in_flight` | Requests open; a server with any is never evicted |
| `encoding` | `utf-8` or `utf-16`, negotiated |
| `capabilities` | Snapshot; only `diagnosticProvider` and `positionEncoding` are read |
| `docs` | URI → `{version, sha256, language_id}`, at most 256 |
| `diagnostics` | URI → `{version?, items, received_at}` |
| `file_snapshot` | Path → mtime, for R7 |

### State transitions

```text
              spawn ok                 initialize answered, no progress for 1 s
 (none) ───────────────► starting ───────────────────────────────────────► ready
    │                     │  │                                               │
    │ command not found   │  │ exits before answering                        │ crash
    ▼                     │  ├─ "not installed" pattern ─► missing           ▼
 missing ◄────────────────┘  └─ otherwise ───────────────► failed ◄── restart limit (3 in 10 min)
    ▲                                                        │          otherwise: back to starting
    └──── "Check again" re-resolves ────  "Restart" ─────────┘
```

Removal from the map (with `shutdown`, `exit`, then kill): switch off, project removed, worktree
deleted, agent deleted, folder gone, override changed, idle 10 min, eviction, runtime shutdown.

`missing` and `failed` entries stay listed until restarted, the switch is toggled, the override changes
or the process restarts. They hold no process and do not use a capacity slot.

## 5. Turn context

`ProjectTurnContext` gains:

| Field | Notes |
|---|---|
| `lsp_enabled: bool` | Read once at turn start |
| `lsp_overrides: Arc<HashMap<String, ServerOverride>>` | Read once at turn start, empty when off |
| `lsp: Arc<LspHost>` | `deps.lsp`; `Default` starts nothing until called |

## 6. Wire values (not stored)

### `LspStatusSnapshot`

```json
{
  "project_id": "p3xY9aQ2mZ",
  "enabled": true,
  "capacity": { "in_use": 3, "max": 6 },
  "servers": [
    {
      "language": "rust",
      "name": "Rust",
      "root": "app",
      "state": "ready",
      "detail": null,
      "reason": null,
      "install_hint": null,
      "overridden": false
    }
  ]
}
```

`root` is relative to the project folder, with `.` for the folder itself. A worktree's server reads
`.vizier/<agent>/<name>/…`. `overridden` is the only override fact non-managers receive.
`capacity` is instance-wide. It contains counts only, so it reveals nothing about other projects.

### `DiagnosticsResult`

See `contracts/lsp-tools.md`.
