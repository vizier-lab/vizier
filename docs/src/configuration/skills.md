# 2.10 Skills

A skill is a reusable instruction package an agent can load on demand: a folder with a `SKILL.md` (YAML frontmatter + markdown body) and optional **resources** (templates, references, scripts). Skills live on disk, are indexed by the agent's embedding model, and are surfaced to the agent as recommendations.

```
skills/
  code-review/
    SKILL.md
    .meta.json                  # written by `vizier skill install`
    resources/
      checklist.md
    scripts/
      lint.sh
```

## `SKILL.md`

```markdown
---
name: code-review
author: vizier
description: Guidelines for conducting thorough code reviews
keywords: [review, quality, security, code, pr]
version: 1
---

# Code Review Skill

## Checklist
- [ ] Readable and well-organized
- [ ] Input validation present
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | yes | — | Slug; must match the folder name |
| `author` | string | yes | — | Who wrote it |
| `description` | string | yes | — | One-liner. Used for recommendation and shown in listings |
| `keywords` | array | no | `[]` | Extra terms folded into the embedding text |
| `version` | number | no | `1` | Bumped by `update_skill`; compared by `vizier skill update` |

> The `activation` field from earlier versions (`always` / `on_demand` / `contextual`) is **no longer used** and is ignored if present. All skills are on-demand; see [Recommendation](#recommendation) below for how the agent finds them.

Resources are every file in the folder except `SKILL.md` and `.meta.json`, listed recursively with paths relative to the skill folder.

## Locations and precedence

| Scope | Path | Visible to |
|-------|------|------------|
| Global | `<workspace>/skills/<slug>/` | every agent |
| Agent | `<workspace>/agents/<agent_id>/skills/<slug>/` | that agent only |

When both scopes have the same slug, the agent-scoped skill wins.

## Recommendation

There is no keyword matching or auto-injection. Instead:

1. Whenever a skill is created, updated, deleted, or the agent boots, its `name + description + keywords` is embedded with the agent's embedding model and stored in the vector index (`skills` context). The body is **not** embedded — write descriptions and keywords that match how users will phrase requests.
2. On every user message, the message is embedded and the top skills above a similarity threshold (≤10, cosine ≥ 0.5) are listed in a `# Possibly Related Skills` system section with their slug and description.
3. The agent decides whether to call `use_skill` to load the full instructions.

Agents without an embedding/indexer get no recommendations but can still `list_skills`.

## Agent tools

| Tool | Description |
|------|-------------|
| `list_skills` | Names + descriptions of global and agent skills (optionally filtered by keyword) |
| `get_skill_details` | Full metadata + resource list |
| `use_skill` | Load a skill's full content into context |
| `read_skill_resource` | Read a resource file (`slug`, `path`) |
| `execute_skill_resource` | Run a script resource (`slug`, `path`, `args`). Interpreter is chosen by extension — `.sh`→`sh`, `.py`→`python`, `.js`→`node`, `.rb`→`ruby`, `.pl`→`perl`, anything else executed directly. **Runs on the host as the Vizier process**, not inside the agent's Docker shell. |
| `create_skill` | Write a new skill into the agent's private scope |
| `update_skill` | Update content / description / keywords / resource files |
| `delete_skill` | Delete a skill and its resources |

`create_skill`, `update_skill`, `list_skills`, `get_skill_details`, and `use_skill` are also available during the [dream cycle](./agents.md#dream-cycle).

## Managing skills

### WebUI

Agent → **Skills** page: list, create, edit, and delete both global and agent-scoped skills, with a markdown editor.

### HTTP API

Global:

| Method | Path | Body |
|--------|------|------|
| `GET` | `/api/v1/skills` | — |
| `POST` | `/api/v1/skills` | `{ "name", "description", "content", "keywords"? }` |
| `GET|PUT|DELETE` | `/api/v1/skills/{slug}` | `PUT`: `{ "description"?, "content"?, "keywords"? }` |
| `GET` | `/api/v1/skills/{slug}/resources` | — |
| `GET` | `/api/v1/skills/{slug}/resources/{path}` | — |

Agent-scoped: the same `GET|POST` / `GET|PUT|DELETE` under `/api/v1/agents/{agent_id}/skills[/ {slug}]`.

### CLI

```sh
vizier skill install code-review                     # registry: skills/code-review in vizier-lab/vizier
vizier skill install someone/their-skills            # GitHub shorthand → https://github.com/someone/their-skills.git
vizier skill install https://git.example.com/x.git   # any git URL (all skill folders in the repo are installed)
vizier skill install ./my-skill                      # local directory
vizier skill install code-review -a my-agent         # into an agent's private skills dir

vizier skill list
vizier skill uninstall code-review [-a my-agent]
vizier skill update code-review                      # registry-installed skills only
```

`git` must be installed for registry and git sources. The registry is `https://github.com/vizier-lab/vizier.git` (its `skills/` directory); currently it ships `skill-maker`, a guide for writing skills.

### `.meta.json`

Written next to `SKILL.md` on install so `update` knows where it came from:

```json
{
  "source": "registry",
  "registry_url": "https://github.com/vizier-lab/vizier.git",
  "slug": "code-review",
  "installed_version": 1,
  "installed_at": "2026-01-15T10:00:00Z"
}
```

`source` is one of `registry`, `git`, `local`, `created` (made by an agent or the API). Only `registry` skills can be updated from the CLI.

## Writing good skills

Install `skill-maker` and ask your agent to use it — it contains the house style for descriptions, keywords, and resource layout so that recommendations fire when they should.
