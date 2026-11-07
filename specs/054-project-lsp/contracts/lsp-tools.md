# Contract: Agent Tools

Five entries in `projects::tools::REGISTRY`, category `coding`, `Access::Read`, with a new
`needs_lsp: true` on the spec. They are called from `execute_python` like every coding tool, so they
add no tool definitions to the prompt. Offered in Plan and Build when the folder is available and the
project's switch is on.

## Common rules

**Position input** (`definition`, `references`, `hover`):

```json
{ "path": "src/lib.rs", "line": 12, "symbol": "resolve", "occurrence": 1 }
```

`line` is 1-based. `occurrence` is optional, 1-based, and defaults to the first match on the line.

**Location output**:

```json
{ "path": "src/lib.rs", "line": 12, "character": 8, "preview": "pub fn resolve(cwd: &Path)" }
```

`path` is relative to the chat's folder. `line` and `character` are 1-based, and `character` counts
Unicode scalar values. A location outside the project is:

```json
{ "outside_project": true, "file": "option.rs", "line": 571 }
```

Locations under `.vizier/` are dropped.

**Limits**: 50 items and 16 KiB. Beyond that: `"truncated": true` and a `"hint"` on how to narrow it.

**Results, not errors**: a question that reaches a server but cannot be answered returns a `status`, so
a script can branch on it:

| `status` | Meaning | Extra fields |
|---|---|---|
| `ok` | Answered | Tool-specific |
| `lsp_starting` | Still indexing; try later | `detail`, e.g. `indexing 41%` |
| `lsp_unavailable` | Server missing | `server`, `install_hint` |
| `lsp_failed` | Crashed past the limit | `reason` |
| `lsp_timeout` | No answer in 10 s | |
| `lsp_busy` | At capacity, nothing can be evicted | |
| `lsp_disabled` | The switch went off during this turn | |
| `no_language` | No server for this file type | |
| `symbol_not_on_line` | The symbol is not on that line | `line_text` |
| `file_unsupported` | Over 2 MiB, or not UTF-8 | |

**Errors**, as with other project tools:

- `path_outside_project`, `not_a_file` from `path::resolve`.
- `tool_not_offered: lsp_hover is not available while language servers are off`, from `refusal()`.

## `lsp_diagnostics`

Input: `{ "path": "src" }`. `path` is a file or folder; empty means the chat's folder.

```json
{
  "status": "ok",
  "freshness": "current",
  "errors": 1,
  "warnings": 0,
  "items": [
    {
      "path": "src/lib.rs",
      "line": 3,
      "character": 21,
      "severity": "error",
      "message": "expected u32, found &str",
      "code": "E0308",
      "source": "rustc"
    }
  ],
  "truncated": false
}
```

`freshness` is `current` or `pending`, as in R6. A file asks the server directly. A folder reports
what the running servers under it have published, so it covers files the servers have analysed, and
adds `"scope": "reported"`. Errors sort before warnings. Hints and info are left out.

## `lsp_definition`

Input: position. Output: `{ "status": "ok", "locations": [Location] }`.

## `lsp_references`

Input: position, plus `include_declaration` (default `false`). Output:
`{ "status": "ok", "locations": [Location], "truncated": false }`.

## `lsp_hover`

Input: position. Output: `{ "status": "ok", "signature": "pub fn resolve(...)", "docs": "..." }`.
Markdown is kept as is. Docs are capped at 4 KiB.

## `lsp_symbols`

Input: `{ "path": "src/lib.rs" }` for a file, or `{ "query": "Resolver", "path": "crates/x" }` for a
search. `path` selects the root and is required with `query`, because a repository can hold several
roots and a search reaches only one.

Output: `{ "status": "ok", "symbols": [{ "name": "resolve", "kind": "function", "container": "Paths",
"path": "src/lib.rs", "line": 12 }], "truncated": false }`.

## Added to `edit_file` and `write_file`

The existing output is unchanged, plus an optional `diagnostics` field:

```json
{
  "path": "src/lib.rs",
  "replacements": 1,
  "first_line": 2,
  "diagnostics": {
    "status": "current",
    "errors": 1,
    "warnings": 0,
    "items": [],
    "other_files": [{ "path": "src/other.rs", "errors": 1, "items": [] }],
    "truncated": false
  }
}
```

| `status` | Meaning |
|---|---|
| `current` | Reflects this edit, and the server has no work in progress |
| `pending` | No answer in 3 s, or work is still running. Items so far may be listed. **Not a claim that the file is clean.** Call `lsp_diagnostics` later |
| `unavailable` | Adds `message`, naming the missing or failed server and its install hint |

The field is **absent** when the switch is off or the file type has no language. A write never fails or
is refused because of this, and the read-before-edit check and `atomic_write` are unchanged.

## Prompt

- **`## Project` (system prompt, stable)**: one fixed sentence, always present in a project chat:
  use `lsp_*` for definitions, references and types when offered, prefer them over `grep`, and read
  `diagnostics` on an edit's result.
- **`## Project state` (per message)**: `Language servers: on.` or `Language servers: off.` Never a
  running list, so the block stays small.
