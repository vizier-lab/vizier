# Contract: Project Skills

**Feature**: `041-project-coding-workspace` (FR-035–FR-043)

A project folder can carry skills of its own, in the layout the Agent Skills ecosystem already
writes: `<root>/skills/<slug>/SKILL.md` plus any resource files beside it. Vizier reads them where
they lie. They are a third scope beside the workspace's global and per-agent skills, and the only
one that is not in the workspace.

---

## 1. Roots and precedence

| Scope | Root | Whose | Written by a tool |
|---|---|---|---|
| Project | `<cwd>/.agents/skills`, then `<cwd>/.claude/skills` | that project chat and its pieces | never |
| Agent | `<workspace>/agents/<id>/skills` | one agent | `create_skill`, `update_skill`, `delete_skill` |
| Global | `<workspace>/skills` | every agent | the same three tools, from the global scope |

- Precedence, highest first: **project → agent → global**. Inside a project root, `.agents` wins a
  slug that `.claude` also carries — they are the same skill in two places, and listing both would
  show every skill twice.
- `cwd` is the turn's folder: the chat's worktree when it is bound to one, else the project root.
  A worktree is a full checkout, so it carries the skills of **its own branch**; there is no
  root-plus-worktree merge.
- Project skills are readable only in a `Project` chat and in a subagent piece launched from one.
  An ordinary conversation, a scheduled task, a dream, an inter-agent delegation and another
  project's chat all resolve no project scope (FR-036).
- A tool that writes never touches a project root, so an agent cannot edit a repository through a
  skill tool (FR-037).

## 2. `SKILL.md` frontmatter

Read into `SkillFrontMatter`. Fields, with what a real skill of the `github/spec-kit` and
`impeccable` families carries:

| Field | Type | Required | Default | Note |
|---|---|---|---|---|
| `name` | string | yes | — | The folder name wins over this field. |
| `description` | string | yes | — | One line; shown in listings and the prompt. |
| `author` | string | **no** | `""` | Was required; 15 of 15 skills in this repo omit it. |
| `version` | number **or** string | no | `1` | `version: 4.7.0` must parse. |
| `keywords` | array | no | `[]` | Extra embedding text (workspace scopes only). |
| `user-invocable` | bool | no | `true` | `false` = not runnable by `/name`. |
| `disable-model-invocation` | bool | no | `false` | `true` = not listed in the prompt. |
| `argument-hint` | string | no | — | Shown after the name in the prompt. |

- Unknown keys (`metadata:`, `compatibility:`, `license:`, …) are ignored by serde. Nothing
  rewrites a project's `SKILL.md`, so no catch-all map is needed.
- A folder whose `SKILL.md` is missing or unparsable is **reported**: one `tracing::warn!` per
  folder, and a count in `list_skills`' output. It is never silently dropped (FR-039).
- `SKILL.md` is not a resource; `.meta.json` (written by `vizier skill install`) is not either.

## 3. The tools

All four are reached from `execute_python` like every tool (code mode), under category `skills`.

| Tool | Project scope | Agent / global scope |
|---|---|---|
| `list_skills` | lists project, then agent, then global; a slug appears once | unchanged |
| `use_skill` | content + folder + resources + how to use them | content only, as today |
| `get_skill_details` | metadata of the resolved skill | unchanged |
| `read_skill_resource` | text resource of the resolved skill | unchanged |
| `execute_skill_resource` | **refused** (`project_skill_resource: …`) | unchanged |
| `create_/update_/delete_skill` | not offered a project root | unchanged |

`use_skill` in a project chat returns:

```
<the skill's content, as today>

## Skill folder
.agents/skills/speckit-plan

Read a file of this skill with `read_file(path=".agents/skills/speckit-plan/<path>")`, or
`glob` to find one. Run a script of it with `workspace_shell`, in the project folder; that is
Build mode only.

Resources:
- scripts/setup-plan.sh
- templates/plan-template.md
```

The folder is the real path relative to the project root, because these bodies refer to their own
location (`<skill-base-dir>/scripts/impeccable`) and to harness paths that must be mapped by hand.

## 4. Resources

- A resource `path` is relative, contains no `..`, does not start with `/`, and resolves inside the
  skill's own folder — the existing `validate_resource_path`, applied on read as well as on create.
- The listing is capped: at most **200 files** and **256 KiB** of paths, binaries and the folders
  `.git`, `node_modules`, `.venv`, `target` skipped. A cut listing says how many were left out.
- `read_skill_resource` refuses a binary file with `binary_resource: <path> is not text` rather
  than returning mojibake.
- `execute_skill_resource` is refused in a project chat:
  `project_skill_resource: run skill scripts with workspace_shell in a project chat`. Its
  `std::process::Command` has no cwd, no timeout and no mode gate; `workspace_shell` has all three
  and is `coding` category, Build-mode only (contracts/coding-tools.md §1).

## 5. The prompt

`## Project Skills`, in `project_md` beside `## Project Instructions` — both are folder content,
read at turn start, and both cost one prompt-cache miss if they change mid-chat.

```
## Project Skills

Skills this project carries, in `.agents/skills`. Load one with
`use_skill(slug="…")` in an `execute_python` script and follow it.

- **speckit-plan** — Execute the implementation planning workflow… (takes: Optional guidance for the planning phase)
- **impeccable** — Use when the user wants to design, redesign… (takes: [shape · audit|critique …] [target])
- **speckit-git-commit** — Auto-commit changes after a Spec Kit command completes (not runnable by name)
```

- One entry per skill: `**slug**`, the description capped at 400 characters, the `argument-hint`
  in parentheses when present, `(not runnable by name)` when `user-invocable: false`.
- A skill with `disable-model-invocation: true` is left out entirely.
- At most 40 entries; beyond that a final `- … and N more (see `list_skills`)` line.
- Omitted when the project carries no skills, when the folder is unavailable, and in every
  non-project turn.
- The section is byte-identical for the life of a chat unless the skills in the folder change; the
  volatile facts (mode, branch, shell) stay in `## Project state`.

## 6. Hooks (FR-043)

Two unrelated things are called hooks; the prompt names both honestly.

**Spec-kit extension hooks** (`.specify/extensions.yml`) — `before_*` / `after_*` entries with
`extension`, `command`, `enabled`, `optional`, `condition`. They are **instructions in a skill's
body**, not automation: the agent reads the YAML with `read_file`, and runs the command it names
with `workspace_shell`. In this repo all 18 are `optional: false` and all commit. Therefore:

- the prompt says these are instructions the agent may follow, and that nothing enforces them;
- a mandatory pre-hook's command is run only in **Build** mode with the shell on;
- before running a commit hook the agent announces what it is about to stage and commit — `git add .`
  over a person's working tree is never silent;
- in **Plan** mode the agent states that the hook was skipped and why, rather than reporting work
  it did not do;
- `condition` is never evaluated; an entry with a non-empty condition is skipped and said to be.

**Harness hooks** (`.claude/settings.local.json`, `.codex/hooks.json`, `.cursor/hooks.json`,
`.github/hooks/*.json`, `.gemini/settings.json`) — a harness runs a command on a session, edit or
stop event and injects its stdout as context. Vizier reads none of them, and its own hook chain
(`crates/vizier-core/src/agents/hook/`) is Rust and hardcoded. The prompt says so, so an agent never
claims a detector ran. Executing harness hooks is a follow-up feature (consent, replay, redaction,
Plan-mode gate), out of scope here.

## 7. Verification

Prose can only be verified by behaviour, so one quickstart step covers it (§9 of
[../quickstart.md](../quickstart.md)): with a fixture project folder holding `.agents/skills`, ask
for `/skills` (all scopes, each slug once), load a project skill and read a resource from it, then
check the hook behaviour in Build mode (announced) and Plan mode (skipped and said to be).
