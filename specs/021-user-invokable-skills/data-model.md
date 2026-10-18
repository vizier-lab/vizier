# Data Model: User-Invokable Skills

**Feature**: `021-user-invokable-skills`

No new tables, columns or files. All types below are in-memory.

## Invocation (parse result)

`skill::invocation::parse(text) -> Parsed`

| Variant | Meaning |
|---|---|
| `Plain` | Not a skill command; handled as ordinary chat |
| `List` | `/skills` (also `/skills@bot`) |
| `Invoke { name, input }` | `/<name> input…` or `/skill <name> input…`; `name` lower-cased, `@bot` suffix removed, `input` trimmed (may be empty) |
| `Unknown { name }` | command-shaped token (R5), resolved later against the skills; becomes `Invoke` if found, else not-found |

Parsing is pure and independent of which skills exist; resolution is a separate step in `VizierSkills`.

## Resolution

`VizierSkills::resolve(name) -> Option<Skill>`: agent skill first, then global (same precedence as `get_skill_content`). `VizierSkills::available() -> Vec<(name, description)>`: the merged list used by `/skills` and the endpoint.

## Turn inputs

- `chat`/`chat_turn` gain `invoked: Option<Skill>`; `context_md` gains the same and renders `## Invoked Skill` (name, description, content, resources) ahead of the memory and recommendation sections.
- The saved history `Request` keeps the original text; `metadata.invoked_skill = "<name>"` records the invocation (FR-009). The WebUI and channel renderers need nothing extra to show it, since the text is what the person typed.

## Validation rules

- A skill name matches the skill's stored name case-insensitively; names `skill` and `skills` are reserved for the commands.
- `input` has no length rule beyond the existing message limits.
