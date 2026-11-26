# Contract: The `[[kind:name]]` Link Grammar

The single text-level contract shared by the engine (parse guard), the WebUI (parse + rewrite), agents (emission), and people (composition). Version: 1 (2026-11-25).

## Forms

| Input | Classification | Renders as |
|---|---|---|
| `[[artifact:NAME]]` | entity link | link to `/:agentId/artifacts?artifact=NAME` |
| `[[function:NAME]]` | entity link | link to `/:agentId/functions/NAME` |
| `[[skill:SLUG]]` | entity link | link to `/:agentId/skills/SLUG` |
| `[[task:SLUG]]` | entity link | link to `/:agentId/tasks/SLUG` |
| `[[kind:TARGET\|LABEL]]` | entity link with alias | link labelled `LABEL` |
| `[[BUNDLE/SLUG]]`, `[[BUNDLE]]`, `[[SLUG]]` | memory link (existing) | unchanged behaviour |
| `[[flavour:x]]`, `[[artifact:]]`, `[[artifact:x]]` inside code spans | not an entity link | literal text |
| `[[artifact:x` (unclosed) | not a link | literal text |

`kind` matching is exact and lowercase. `TARGET` may not contain `]`, `|` (after the first alias separator), or a newline; surrounding whitespace is trimmed. `agentId` is always the agent of the conversation/document the text appears in — links are agent-scoped by context, never absolute.

## Precedence rules

1. Code spans/fences win: no linkification, no suggestions inside them (both sides already split code out first).
2. A `[[...]]` whose pre-colon word is a reserved kind is an entity link; the memory classifiers (`parse_wikilinks`/`classify_relation` in `memory_bundle.rs`) **skip** it entirely — it must not create, break, or heal a memory relation.
3. Everything else in `[[...]]` is a memory link with today's exact behaviour (byte-identical classification).

## Engine side (vizier-core)

- `parse_wikilinks` (and the relation classifier that consumes it) filters out targets matching `^(artifact|function|skill|task):` before classification.
- No other engine parse change: memory markdown links, passage indexing, and the relations graph are untouched.

## WebUI side (webui)

- One module (`lib/entityLinks.ts`) exposes: `parseEntityLinks(text)`, `rewriteEntityLinks(text, ctx)` (string→string, code-span aware, resolving hrefs from the kind table), and the kind→href table. `rewriteEntityLinks` composes with (does not replace) `linkWikiLinks` in `MemoryDocument`; `MessageList` applies it to reply and message text before `<Markdown>`.
- Broken state: target absent from the per-agent link-targets cache → broken styling, no navigation. Unknown cache → render as a normal link (optimistic); the destination page owns not-found.
- Shared snapshots / offline viewer: entity links render as plain text (no live-resolvable destination) — the existing sign-in rule covers the online viewer case.

## Versioning

Additive only: new kinds append to the reserved word list; existing forms never change meaning. The engine guard list and the WebUI kind table must stay in sync — the conformance fixture set in `webui/app/lib/entityLinks.test.ts` pins it.
