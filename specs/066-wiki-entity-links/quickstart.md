# Quickstart: Entity Wiki Links (066-wiki-entity-links)

Offline verification per the constitution's e2e gate: a running binary with an agent on the `dummyplug` provider (no keys, no network). The WebUI rendering and composer behaviour are covered by vitest fixtures (`just web-check`) because they are browser-side; the dummyplug steps cover everything agent-observable over the wire. The one live-provider step (prompt adherence, SC-005) is marked as such.

## Setup

```sh
just build          # webui into webui/build/client (build.rs needs it)
just run            # dev.vizier.yaml — dummyplug agent(s) configured
```

Let `A` be a dummyplug agent id (`vizier agent ps` or `GET /api/v1/agents`). Authenticate as an all-agents manager (existing dev credentials).

## Steps

### 1. Give the agent one entity of every kind (HTTP)

```sh
# artifact
curl -F name=report -F file=@README.md localhost:9999/api/v1/agents/$A/artifacts
# function (custom tool)
curl -X POST localhost:9999/api/v1/agents/$A/custom-tools -d '{"name":"fetch_weather","category":"custom","script":"pass","input_schema":{}}'
# task
curl -X POST localhost:9999/api/v1/agents/$A/tasks -d '{"slug":"weekly-report","title":"Weekly report","instruction":"write it","schedule":{"cron":"0 9 * * 1"}}'
# skill (workspace file)
mkdir -p "$VIZIER_WORKSPACE/agents/$A/skills/summarize" && printf -- '---\nname: summarize\ndescription: Summarize text\n---\nDo the thing.' > "$VIZIER_WORKSPACE/agents/$A/skills/summarize/SKILL.md"
# memory doc
curl -X PUT localhost:9999/api/v1/agents/$A/memory/doc/research/notes -d 'Notes body'
```

### 2. The link-targets endpoint serves all five targets

```sh
curl localhost:9999/api/v1/agents/$A/link-targets
```

Expect `artifacts[].name == "report"`, `functions[].name == "fetch_weather"`, `tasks[].slug == "weekly-report"`, `skills[]` containing `summarize` (scope `agent`), `memory.bundles` containing `research`, `memory.docs` containing `research/notes`.

### 3. Links ride a durable turn intact (dummyplug)

Open the chat WebSocket (or the WebUI) and send the agent the literal text:

```
Cite your work: [[artifact:report]], [[function:fetch_weather]], [[skill:summarize]], [[task:weekly-report]], [[research/notes]], [[flavour:vanilla]]
```

dummyplug echoes the text back as the reply. Expect:

- The reply's history entry (and the stored `session_history` row) carries the text **unchanged** — links are plain text; no server-side rewriting or stripping (FR-9/FR-10).
- `GET …/topic/{topic}/history` shows the same literal text; a reload/restart changes nothing (no migration).

### 4. The engine does not misread entity links as memory relations

After step 3, `GET …/memory/doc/research/notes` relations — and the memory graph for the agent — contain **no** broken `artifact:report`-style bundle edge from the guard change (the reply text contained `[[artifact:report]]`, which the classifier must skip). Existing memory wikilinks in the same transcript (`[[research/notes]]`) still classify as before.

### 5. Broken and permission edges (curl only)

- `curl localhost:9999/api/v1/agents/$A/link-targets` after deleting the artifact → `artifacts` no longer lists `report`; the WebUI fixture set (vitest) renders the link broken from exactly this response shape.
- A scoped-down user (view-only, not shared on the agent) gets `403` from link-targets; an unknown agent id gets `404`.

## WebUI checks (vitest, not dummyplug)

`just web-check` must pass with the new fixtures in `webui/app/lib/entityLinks.test.ts`: every kind renders the right href; alias honoured; code-fence contents untouched; unknown kind literal; broken target styling from a stale index. Composer fixtures cover the `[[` trigger, kind-prefix narrowing, and Escape/insert behaviour.

## Live-provider step (marked: needs a real model)

SC-005 (≥90% of emitted references are valid links) is measured once against a live provider: ask the agent to "cite the artifact, function, skill, and task you have" and count resolvable `[[kind:…]]` occurrences in the reply. Offline dummyplug cannot measure prompt adherence.
