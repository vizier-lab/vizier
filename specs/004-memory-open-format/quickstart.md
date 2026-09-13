# Quickstart: verifying bundle-backed memory

Per the constitution's "Manual verification for runtime-affecting changes" (the automated test
suite is sparse; this touches storage migrations and process startup), exercise the actual
binary, not just `cargo build`/`cargo test`.

## 1. Fresh-install agent memory lifecycle (Story 1)

```sh
just install
just run-d
```

- Through the WebUI (or the agent directly), have an agent write a memory without naming a
  bundle. Confirm on disk: `{data-dir}/agents/{agent}/memory/default/{slug}.md` exists, with
  frontmatter and no `visibility`/`shared_to` fields.
- Have it write a second memory naming a new bundle, e.g. `andy`. Confirm
  `agents/{agent}/memory/andy/` was created automatically with its own `index.md`/`log.md`.
- Link the two: content in one memory referencing `[[andy]]` or `[[andy/some-slug]]`. Call
  `memory_follow`/`GET /{bundle}/related` and confirm the cross-bundle link resolves.
- Write a memory to a nested path, e.g. `friends/bred`, inside a bundle. Confirm the
  subdirectory and its file are both created.
- Attempt to write a second memory to an already-used `(bundle, path)` — confirm it's rejected
  with a clear error, not silently overwritten.

## 2. Human readability (Story 2)

- Open a concept document directly in a plain text editor (outside Vizier). Confirm title, tags,
  content are legible without decoding.
- Hand-edit that file's content (fix a fact), save, then have the agent read/query that memory
  again — confirm it sees the edit.
- Open a bundle's `index.md` directly — confirm it lists the bundle's current concepts without
  needing to open each one.
- In the WebUI, open the memory graph: confirm the top-level view shows bundles as nodes, and
  clicking one switches to that bundle's concept-level graph.

## 3. Portability (Story 3)

- Stop the instance (`just shutdown`). Copy `agents/{agent}/memory/` to a fresh workspace/data
  dir for a new instance of the same agent config. Start the new instance and confirm
  search/list/graph reflect the same content, then write a new memory there and confirm no
  id/slug collision with the migrated ones.

## 4. WebUI export/import (Story 4)

- From the WebUI, export a bundle with several memories + at least one attachment as `.zip`.
  Confirm the archive opens in a standard OS zip tool and contains concept documents, index/log,
  and the attachment.
- Import that `.zip` back, either as a new bundle name or into a fresh agent. Confirm every
  concept document, its metadata, and its attachment are present and addressable identically to
  before export.
- Re-import the same `.zip` into a bundle that already has a colliding concept path — confirm
  the operator is prompted for a destination name and the colliding concept is reported/skipped,
  not overwritten.

## 5. Migration (existing installs, both backends)

- Against a pre-existing workspace/data dir created before this feature with the **sqlite**
  backend (rows in the legacy `memory` table), start the upgraded binary once. Confirm:
  - Every prior memory is now under `agents/{id}/memory/default/` with `index.md`/`log.md`
    present, and readable via `memory_list`/`GET /` (served from the Memory Graph Index).
  - No `visibility`/`shared_to` remain in any frontmatter.
  - A memory that was previously `global`/`shared` is now private and readable by exactly the
    agent it was migrated to (check `tracing` output for the "which agent" decision if it was
    ambiguous).
  - Restarting the binary again does not re-run or duplicate either migration.
- Separately, against a pre-existing workspace/data dir created with `--storage filesystem`
  (flat `agents/{id}/memory/*.md`, plus agents/tasks/sessions/users stored as files), start the
  upgraded binary once with the same config. Confirm:
  - The startup log includes the `tracing::warn!` telling the operator their `filesystem`
    storage setting is no longer accepted going forward.
  - Every entity that was on disk under the filesystem backend (agents, tasks, sessions, users,
    memories) is now readable from the sqlite-backed instance with no data loss.
  - A subsequent run with `--storage filesystem` (or `VIZIER_STORAGE=filesystem`) fails fast with
    a clear "no longer supported" error rather than silently falling back or crashing deep in
    startup.

## 6. Memory Graph Index cache correctness

- With an agent that has several memories across bundles, call the listing/filter/graph
  endpoints and confirm they return correct results with the `DocumentStore` untouched (e.g.,
  temporarily rename the underlying `LocalDocumentStore` directory read-only or instrument
  logging to confirm no `get`/`list` calls happen on the listing path, only on `memory_detail`).
- Hand-edit a concept document's title directly on disk (bypassing Vizier), then call the
  bundle's listing/index endpoint. Confirm the stale cached title is detected and corrected
  (re-read once, then cache-only again) rather than served indefinitely.

## 7. Regression gates

```sh
cargo clippy
cargo test
cd webui && npm run typecheck
```
