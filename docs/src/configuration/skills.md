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
2. On every user message, the message is embedded and the top skills above a similarity threshold (≤5, cosine ≥ 0.4) are listed in a `## Possibly Related Skills` section of the message's `# Context` block with their slug and description.
3. The agent decides whether to call `use_skill` (from an `execute_python` script) to load the full instructions. Its BOOT.md also tells it to check for a skill before multi-step or repeated work.

Agents without an embedding/indexer get no recommendations but can still `list_skills`.

## Invoking a skill

A person can run a skill by name instead of waiting for the agent to pick it. The message is ordinary chat on every channel:

| Message | What happens |
|---------|--------------|
| `/<skill> [input]` | Runs the skill. `input` is the message the agent works on; with none, the agent works from the conversation. |
| `/skill <name> [input]` | The same, for a skill whose name is also a platform command. |
| `/skills` | Lists the skills you can run, one per line with its description. |
| `/<unknown>` | `No skill named "<unknown>".`, up to three close names, and a `/skills` hint. |

Names match case-insensitively, and `_` and `-` count as the same (`/release_notes` runs `release-notes`). An agent skill wins over a global one with the same name. Built-in commands (`/abort`, `/checkpoint`, `/new`, …) always win; reach a skill with such a name through `/skill <name>`. `skill` and `skills` cannot be skill names for invocation. Text that only starts with a slash, such as `/etc/hosts`, stays ordinary chat.

The agent receives the skill's full instructions (and its resource list) in an `## Invoked Skill` section of the message's `# Context` block, and the input as the message itself. History keeps what the person typed, with `invoked_skill` in the request metadata. `/skills` and the not-found reply are answered directly: no model call, no usage recorded. Installing or removing a skill takes effect on the next message, with no restart.

**WebUI.** Typing `/` in the composer suggests the built-in commands and every skill the agent can run (from `GET /api/v1/agents/{agent_id}/skills/available`). Picking a skill puts `/<name> ` in the composer for you to add input.

**Discord.** Each skill is a slash command of the agent's bot, with an optional `input` option, so it runs from the command picker without a mention, and the picker shows which bot it belongs to. The bot also has `/skill` (the `name` option autocompletes the agent's skills) and `/skills`. A skill gets no command of its own, and is logged once, if its lower-cased name is not 1–32 characters of `a-z`, `0-9`, `-` or `_`, if it equals a built-in command, or once the bot has Discord's limit of 100 commands; `/skill` still runs it. The commands are re-checked every 60 seconds and re-registered only when the list changed. Typed text works in a DM or after a mention (`@bot /release-notes v1.2`).

**Telegram.** The bot's command menu lists the built-ins, `/skill`, `/skills` and the skills, with `-` shown as `_` (Telegram allows only `a-z`, `0-9` and `_`), re-checked every 60 seconds. In a group, `/<skill> …` or `/skill <skill> …` runs without a mention when it names one of the agent's skills; `/skills` and unknown names need the bot addressed (`/skills@mybot` or a mention), so several bots in one group do not all answer. The bot only sees group messages that are not addressed to it when its privacy mode is off (BotFather → `/setprivacy`).

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

`GET /api/v1/agents/{agent_id}/skills/available` returns `[{ "name", "description" }]` for every skill the agent can run (its own and the global ones, agent winning, sorted by name), without content.

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

## Project skills

A third scope lives in the **project folder** itself, in the layout the Agent Skills ecosystem already writes, so a repository that carries skills for other coding agents works here unchanged:

| Root (inside the project) | Read when |
|---|---|
| `.agents/skills/<slug>/SKILL.md` | first |
| `.claude/skills/<slug>/SKILL.md` | second, for a slug `.agents` does not have |

A project skill is available in a [project chat](projects.md) and in a subagent piece launched from one, and nowhere else: an ordinary conversation, a task, a dream and another project's chat all resolve no project scope.

- **Precedence**: project → agent → global, and `.agents` before `.claude` inside a project. A slug appears once, so a skill installed in both folders (`this repository` has all 15 in both) is one line in `/skills`, not two.
- **Read where it lies, never indexed.** Project skills are not embedded and never recommended by similarity: they are listed in the project chat's system prompt (`## Project Skills`) and read from disk each time one is loaded, so a branch switch shows up on the next turn and one project's skills can never surface in another.
- **Never written by a tool.** `create_skill`, `update_skill` and `delete_skill` stay in the workspace; an agent cannot edit a repository through a skill tool.
- **Scripts run through the shell.** `execute_skill_resource` is refused in a project chat; run a skill's scripts with `workspace_shell`, which stays inside the project folder, needs Build mode, and is timed and capped.
- **In the composer.** Typing `/` in a project chat offers the project's skills alongside the workspace ones (the project copy wins a slug), read from the chat's own folder — its worktree when it is bound to one, so the menu follows the branch. A skill with `user-invocable: false` is left out of the menu (it cannot be run by name) but is still in the prompt. They are never on the **Skills** page, which stays workspace-scoped.

### Frontmatter

Everything Vizier writes, plus what the ecosystem writes:

| Field | Type | Default | Note |
|---|---|---|---|
| `name` | string | — | The folder name wins. |
| `description` | string | — | Listed in `/skills` and in the project prompt, cut at 400 characters there. |
| `author` | string | `""` | Optional: most ecosystem skills omit it. |
| `version` | number or string | `1` | `4.7.0` is accepted and written back unchanged. |
| `keywords` | array | `[]` | Workspace scopes only; not indexed for a project. |
| `user-invocable` | bool | `true` | `false` hides it from `/name` (the agent can still load it). |
| `disable-model-invocation` | bool | `false` | `true` keeps it out of the prompt entirely. |
| `argument-hint` | string | — | Shown after the name, e.g. `[audit|critique] [target]`. |

Unknown keys (`metadata:`, `compatibility:`, `license:`, …) are ignored. A folder whose `SKILL.md` is missing or unparsable is reported — one log line and a count in `/skills` — rather than quietly disappearing.

### Hooks

Two different things are called hooks, and neither needs anything from Vizier:

- **Extension hooks** (`.specify/extensions.yml` and friends) are **instructions inside a skill's body**, not automation: the agent reads the file and, in Build mode, runs the command with `workspace_shell`. Nothing enforces them, so the project prompt tells the agent to announce a commit before making one and to say when a hook was skipped for lack of a shell.
- **Harness hooks** (`.claude/settings.local.json`, `.codex/hooks.json`, `.cursor/hooks.json`, `.github/hooks/*.json`, `.gemini/settings.json`) are run by those tools, not by Vizier, which never reads them: an agent will not report that a design detector ran. Skills that depend on one usually document a manual path (for example `impeccable context`, then `impeccable detect`); run that through `workspace_shell` in Build mode.
