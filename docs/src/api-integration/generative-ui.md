# Generative UI

In a WebUI or project chat, an agent can answer with an **interactive page**: an app, a game, a simulation, a dashboard or a form, drawn inside its reply. The page is plain HTML the agent writes, and Vizier keeps it with its versions.

This is the built-in tool `ui_render` (category `ui`), offered in WebUI and project conversations, whether or not a tab is open. It is not offered in scheduled tasks, dreams, Discord, Telegram, Workspaces, or from a detached script. A successful call ends the turn: the page is the reply.

## How an agent draws a page

- **A new page:** `ui_render(title="…", html="…")` returns `{"status": "rendered", "page": "pg_…", "version": 1, "verified": …}`.
- **A change:** `ui_render(page="pg_…", base_version=N, edits=[{"find": "…", "replace": "…"}])`. Each `find` must match exactly once; the edits apply in order, all or nothing. A full replacement (`html=`) works too. `base_version` must be the latest version, so two writers never overwrite each other.
- **Refusals** are errors whose message is JSON with a `code`: `invalid_input`, `too_large`, `unknown_page`, `other_conversation`, `stale_base`, `edit_not_found`, `edit_ambiguous`, `problems`.
- The tool's guide (page model, rules, look, bridge, an index of the elements and examples) comes back from `describe_tool_function("ui_render")` as `guide`. The details are two more tools in category `ui`, offered wherever `ui_render` is, so a page pays only for what it uses:
  - `ui_describe_elements(names=[…])` returns the attributes, events and an example of each `<vz-*>` element named (`card` and `vz-card` both work; an unknown name comes back in `unknown` with `did_you_mean`).
  - `ui_examples(kind)` returns a whole example page (`dashboard`, `form`, `game`, `worker`, `three`); with no `kind`, the list.

  The three texts live in `crates/vizier-core/assets/ui-kit/` (`REFERENCE.md`, `ELEMENTS.md`, `EXAMPLES.md`), and `scripts/ui-kit-reference.test.mjs` keeps them in step with the kit.

Limits: 1 MiB per version, 100 edits per call, a 120-character title.

## Checking a version

When a tab has the conversation open, each new version is mounted off-screen in that tab for two seconds. Uncaught errors, rejected promises, `console.error`, anything the page policy blocked, an invalid `<vz-*>` attribute, and a page that stops responding are reported back:

- **Clean:** the call succeeds with `verified: true`.
- **Problems:** the version is **saved**, and the call fails with `code: "problems"` and the list, so the agent fixes it with edits in the same turn.
- **No tab, or no answer within 10 s:** the call succeeds with `verified: false` and `reason` `no_tab` or `timeout`; the WebUI shows an "unchecked" chip.

The check travels on the Fleet bus (`ui.{session}.verify`); every chat socket on the conversation answers, and the first report wins (`contracts/verify-protocol.md` in `specs/063-html-generative-ui/`).

A version that **froze the tab** (where a frame shares the WebUI's thread) is drawn paused after a reload, with *Run anyway*, and the agent's next turn in that conversation is told which version froze.

## The page and its kit

Every page starts with the **Vizier kit**:

- **Theme and base CSS:** the design tokens, the font and styles for plain HTML elements (headings, links, buttons, inputs, tables, code), plus the layout classes `vz-stack`, `vz-row`, `vz-grid`. The page follows the person's theme, live. The kit's rules sit in a cascade layer, so a page's own CSS always wins.
- **Elements**, drawn with the WebUI's own components, loaded only when a page uses one: `<vz-card>`, `<vz-stat>`, `<vz-table>`, `<vz-chart>`, `<vz-tabs>`, `<vz-field>`, `<vz-badge>`, `<vz-callout>`, `<vz-progress>`, `<vz-image>`. They are described once in `webui/app/kit/manifest.ts`.
- **Libraries** a page imports by name, with no network: `three`.
- **The bridge, `window.vizier`:** `state.get/set/subscribe` (one JSON value per page, shared by its versions and tabs, kept across reloads), `send(text, data)` (the person's next turn, only within 10 s of their own click or key press and at most once a second), `files.url(name)` (a file of the conversation as a `data:` URL), and `worker(fn)` (heavy work on its own thread, with a time limit).

## Security

A page runs in a frame served from its own address (`/api/v1/ui-frames/{ticket}`, a signed ticket valid for ten minutes for one version) with its own policy:

- the frame has an opaque origin, so it reads no cookie, storage or token of the WebUI and cannot reach the parent page;
- script cannot use the network (`connect-src 'none'`), load remote scripts, styles or fonts, open popups or submit forms;
- images, audio and video may come from any `https:` address; `http:` is blocked;
- the page's raw source is served as `text/plain`.

The WebUI enforces the bridge's rules (who may send, how often, how large the state is), and the server checks them again: a state save or a page event is accepted only for a page of that conversation.

## Older pages

Pages drawn before this tool, by the json-render client tool (`ui` + `render`) and by `render_interactive_ui`, still draw with their state in the conversations that have them. Those names are retired: a call to `ui_components` or `render_interactive_ui` answers with guidance to use `ui_render`, and a WebUI tab that still declares the old client tools has them dropped. The read-only renderers are tracked for removal before 1.0 in [#82](https://github.com/vizier-lab/vizier/issues/82).
