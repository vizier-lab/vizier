# Memory

Each agent's long-term memory is a set of **markdown documents**, grouped into **bundles**, linked into a graph, and indexed for semantic search. The documents live in Vizier's **database** (SQLite or PostgreSQL), which is the source of truth. You read and edit them in the WebUI or over the HTTP API, and you can export any bundle as markdown files.

## Layout

```
core/                       # reserved: holds only soul, cannot be deleted
  soul                      # the agent's identity, injected every turn
default/                    # every agent has a "default" bundle
  project-x
  people/alice              # concepts can be nested in paths
harry-potter/               # any other bundle, created on first write
  hogwarts
  ...
```

**`core` is reserved.** It accepts one document, `soul`, and refuses any other; it cannot be deleted, even with `force`; and a bundle import into it is refused. The document inside it is an ordinary memory — readable, writable, versioned and deletable — and it is the only memory injected into every turn. See [Agents → Core](./agents.md#core).

A **concept** is one markdown document addressed by `(bundle, path)`, where `path` is relative to the bundle and omits the `.md` extension (`people/alice`). Its **slug** is the last part of the path (`alice`). A bundle exists as long as it has at least one concept (`core` always exists).

## Concept document

This is a concept as an export writes it:

```markdown
---
slug: hogwarts
title: Hogwarts — The Magical School
created_at: 2026-09-13T10:50:01Z
updated_at: 2026-09-13T10:50:01Z
agent_id: viz
bundle: harry-potter
tags: []
keywords: []
relations:
- gryffindor.md
- slytherin.md
read_count: 0
---
## Hogwarts

Founded by [Godric Gryffindor](./gryffindor.md) and [Salazar Slytherin](./slytherin.md).
Compare with [[star-wars/jedi]] or the whole [[marvel]] bundle.
```

| Frontmatter | Maintained by | Notes |
|-------------|---------------|-------|
| `slug`, `title`, `bundle`, `agent_id` | Vizier | |
| `created_at`, `updated_at`, `read_count` | Vizier | `read_count` increments when the agent reads the concept via the memory tools |
| `tags`, `keywords` | you / the agent | Filterable in the WebUI and API |
| `relations` | Vizier | Parsed from the body on every write |
| `attachments` | — | Removed: point at an artifact with an entity link (`[[artifact:name]]`, see [Entity links](/docs/configuration/entity-links)). The key in older documents is ignored |

### Links

| Syntax | Meaning |
|--------|---------|
| `[label](./path/to/concept.md)` | Same-bundle link, relative to the bundle root |
| `[[bundle/slug]]` | Cross-bundle link to one concept |
| `[[bundle]]` | Cross-bundle link to a whole bundle (resolves to every concept in it when followed) |
| `[[artifact:name]]`, `[[function:name]]`, `[[skill:slug]]`, `[[task:slug]]` | Entity links — not memory at all: they point at the agent's artifacts, functions (custom tools), skills and tasks. See [Entity links](/docs/configuration/entity-links) |

Links to concepts that don't exist are kept as **broken edges** in the graph and shown as such; they heal automatically when the target is created. A kind-prefixed `[[kind:name]]` is never read as a bundle reference, so citing your work never leaves a broken edge behind.

## How agents use memory

Whenever an agent has an embedding model configured (the default), it can write, search, read, list and delete concepts, follow links and look at the graph, in any bundle except that `core` only ever holds `soul`. An agent searches by meaning, so it finds a passage even when the wording differs.

Before each reply, Vizier also searches memory with your message and shows the best passages to the model automatically, so it often has the right context without asking. This is the agent's **auto context** setting: by default up to 5 passages in a chat, with a similarity threshold of 0.45. Messages the agent only reads silently in group channels use no automatic context unless you raise it.

## Size limits

One memory should be one thing: a search returns a handful of excerpts drawn from across everything the agent knows, so a document covering several subjects competes with itself for those slots. Each agent therefore carries two bounds on a document's **content** (the markdown body — titles, tags and links are metadata and are not counted):

| | hard ceiling | soft limit |
|---|---|---|
| ordinary memory | 8 KiB | 4 KiB |
| the identity document (`core/soul`) | 4 KiB | 1.5 KiB |

- **Over the ceiling** the write is refused, and nothing is written: the response states the size, the ceiling, how far over it is, and a concrete way to split the document into smaller memories that link to one another. Only exceeding the ceiling is refused; a document exactly at it is accepted.
- **Over the soft limit** the write succeeds **in full** and the result carries a separate `warning` field. It is advisory: nothing is truncated, and it is a field rather than a sentence in the message so an agent cannot read a successful write as a failure and retry it.
- **A document already over the ceiling** — written before the limit existed — stays readable in full and accepts a write that makes it shorter, so the limit is never a trap on the documents it exists to fix. The upgrade that carries an existing identity document into `core/soul` bypasses the ceiling entirely, so no upgrade is blocked or made lossy.
- The limits apply to **every** write path: the agent's own `memory_write`, a person's edit, a restore and an import all land in the same place.

They are per-agent (`limits` in the agent config) and take effect on the next write. The operating doctrine deliberately names no figure, so the numbers live only where they are enforced and cannot drift.

## HTTP API

All under `/api/v1/agents/{agent_id}/memory`:

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/` | List concepts. Query: `bundle`, `tags` (comma-separated), `offset`, `limit` (50), `sort_by`, `sort_order` |
| `POST` | `/` | Create: `{ "title", "content", "bundle"?, "path"?, "tags"? }` (attachments are added separately; a non-empty `attachments` field is `422`) |
| `GET` | `/query` | Semantic search. Query: `query`, `bundle`?, `limit` (10), `threshold` (0.5) |
| `GET` | `/bundles` | List bundles |
| `GET` | `/bundles/graph` | Bundle-level graph (`?search=` filters) |
| `GET` | `/{bundle}/graph` | Concept-level graph of one bundle |
| `GET` | `/bundles/{bundle}/export` | Download the bundle as `<bundle>.zip` |
| `POST` | `/bundles/import` | `multipart/form-data` with `file` (or `zip`) and optional `bundle` name |
| `DELETE` | `/bundles/{bundle}` | Delete a bundle |
| `GET|PUT|DELETE` | `/doc/{bundle}/{path}` | Read / update (`{ "title", "content", "tags"?, "attachments"? }`) / delete one concept |
| `GET` | `/related/{bundle}/{path}` | Linked concepts |
| `GET|POST` | `/history/{bundle}/{path}` | Version history of one concept: list (`?offset&limit`), one version (`?seq`), diff (`?to[&from]`); `POST { "seq" }` restores a version as a new one. See [REST API → Version history](../api-integration/rest-api.md#version-history-memories-core-included) |
| `GET|PUT|DELETE` | `/{slug}` | Legacy: same as `/doc/default/{slug}` |
| `GET` | `/{slug}/related` | Legacy related lookup in `default` |

## WebUI

Agent → **Memory**: browse bundles, view the graph, read/edit concepts with a markdown editor, export/import `.zip` bundles. Every concept's view has a **History** button: browse earlier versions (who saved, when, why), see a line diff against the previous version or between any two, and **Restore** an earlier version (or a deleted memory) as a new version. Identity is browsed the same way: `core` appears in the tree marked **reserved**, with its delete action disabled, and `core/soul` is an ordinary document with its own history, diff and restore. Saving from a stale view is refused with the newer content rather than overwriting it.

## Export / import

A bundle export is a zip of markdown files: one `.md` file per concept (frontmatter and body), plus an `index.md` table of the concepts and a `log.md` change list built from the version history. The version history itself is not included; each imported concept starts a fresh history with an `import` revision. Import writes every `.md` in the archive into the target bundle (`bundle` form field, default `default`), skipping `index.md`, `log.md` and any concept whose path is already taken, and embeds them with the agent's current embedding model. Archives exported by earlier versions import as before. Archives with no `.md` files or with unsafe paths are rejected.

## Upgrading from files

Versions before the database stored each concept as a file under `<workspace>/agents/<agent_id>/memory/` (or the same keys in the S3 bucket). The first start after upgrading copies every concept into the database. Nothing is changed or deleted in the files, and no new version history entries are created. If the file store can't be read, or an agent has memory recorded but the store holds none of its files (check the `objects` setting and that the files were copied), the server **still starts**: it logs an error, the memory that was not copied does not show yet, and the copy is tried again on the next start. A concept whose frontmatter can't be read is logged and left out; its file stays where it is, so you can fix it and import it.

Once the upgrade is done, Vizier no longer reads those folders, and **editing them has no effect**. After you have backed up the workspace, you can delete `agents/*/memory/`. To go back to an earlier version, the old files are still there, but memory written after the upgrade is not in them.

## Design notes

- To keep memory in `git` or edit it with your own tools, export a bundle, work on the files, and import them into a bundle.
- Vector embeddings are stored in the shared `document_index` table — see the dimension caveat in [Voice, Images & Embeddings](./tools-embedding.md#embedding-embedding).
- The format comes from [`specs/004-memory-open-format/`](https://github.com/vizier-lab/vizier/tree/master/specs/004-memory-open-format); the move to the database is [`specs/070-memory-in-database/`](https://github.com/vizier-lab/vizier/tree/master/specs/070-memory-in-database).
