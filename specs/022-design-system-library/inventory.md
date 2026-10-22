# Component Inventory: Design System Library

**Feature**: [spec.md](spec.md) · FR-013, FR-013a, FR-013b, SC-002, SC-008
**Source**: FigCraft `Vizier.figcraft`, saved 2026-10-23 00:19 (29 pages, 38,955 layers), read from disk and through the FigCraft MCP
**Status**: **Approved 2026-10-23** with the decisions below (see *Review decisions*).

## How this was made

1. Every component and component set in the file was listed with its variant axes, properties, description and usage count (110 entries: 2 on Brand, 108 on Design System).
2. Usage was counted on the 26 screen and reference pages. The screens place only two Design System components directly: **Sidebar** (46×) and **SettingsNav** (10×). Everything else on them is drawn with plain frames (3,004 of them unnamed). So most components that "exist on the Design System page" also appear on screens as hand-drawn copies, and those copies sometimes go further than the component does. Where they do, the extension is listed against the component (**D** rows).
3. Each dark frame on every screen page was rendered and read, including the 8,000 px Chat "states & flow" board lane by lane: Login, Onboarding, Home ×3, Chat ×4, Core, Memory ×2, Tasks, Skills ×2, Dreams, Usage, Config, New agent, Channels, Addons, MCP, Profile, API keys, Providers, Users, Roles, States, Mobile ×2, Scaling. Light frames were not read one by one, because the screens bind colour to `Color` variables and the light frames switch mode. The token audit below confirms this.
4. Reusable elements that aren't Design System components were extracted (**X** rows) with every frame they appear on, so each can be built from all of its occurrences (FR-013b).
5. A structural pass grouped every repeated layer structure across the dark screens (~100 groups) and checked each against this list. All mapped to an entry.
6. Things that look like components but are page layouts or design-board annotation were excluded (**Excluded**, at the end), so it's clear they were seen and left out on purpose.

**Tiers** set the build order, not importance: **T1** primitives that others depend on, **T2** composites of T1, **T3** domain components (chat, agents, integrations, settings), **T4** visualisation and editor (charts, graph, rich text).

**Replaces** maps a component to the current WebUI file it will stand in for. It is my mapping from `webui/app/components/` and `webui/app/routes/`, not from FigCraft, whose descriptions name no files.

---

## F. Foundations

| ID | Item | Count | Notes |
|----|------|-------|-------|
| F1 | Colour tokens (`Color`, Dark/Light) | 37 public + 1 hidden | `doc/viewport` is hidden from publishing and is **not** exported. The spec's "31" predates the 7 state tokens added on 2026-10-23 (`ink-hover`, `ink-pressed`, `pressed`, `focus`, `failed-hover`, `failed-pressed`, `border-hover`), and will be corrected. Names come from each variable's code syntax, `var(--vz-*)`. |
| F2 | Size tokens (`Size`) | 6 radius + 12 space | radius xs 4 · sm 6 · md 8 · lg 10 · xl 12 · pill 999; space 2–48 |
| F3 | Tone (`Tone`) | 1 variable × 7 modes | muted, default, faint, inverse, failed, live, retrying, each aliasing a Color token; drives `currentColor` |
| F4 | Text styles `Type/*` | 24 | display, title, heading, prose, body(+para/medium/strong), ui(+para/medium/strong), small(+para/medium/strong), label, mono-xs(+medium), mono-sm(+medium), code, code-block, code-medium |
| F5 | Effect styles `Elevation/*` | 2 | popover, dialog |
| F6 | Fonts | 2 | Schibsted Grotesk (UI, wordmark), JetBrains Mono (mono, code) |

## I. Icons

| ID | Item | Count | Notes |
|----|------|-------|-------|
| I1 | `Icon/*` on the Design System page | 47 | Hand-drawn Lucide lookalikes. Every one has a Lucide counterpart (checked side by side on 2026-10-23), so the library uses **Lucide** (`lucide-react`), not the drawn vectors. Mapping below. ⚑ The **descriptions** of `Icon/download` and `Icon/upload` are swapped ("upload icon" / "download icon"). |
| I2 | Glyphs drawn on screens with no `Icon/*` component | 25 | All exist in Lucide: `globe` (MCP remote), `square-terminal` (MCP local), `folder` (bundle), `zoom-in`, `zoom-out`, `maximize` (graph fit, interactive page, editor fullscreen), `play`, `square` (stop), `arrow-down` (jump to latest), `arrow-right` (→ links), `smile-plus` (add reaction), `slash` (composer skills), `bold`, `italic`, `code`, `link`, `list`, `list-ordered`, `list-tree` (outline), `table`, `menu` (Mobile), `layers` (Mobile jobs), `circle-check` (empty state), `ban` (cancelled job), `arrow-up-right` (external in run links). |

**Icon source: Lucide.** The library's `Icon` uses `lucide-react`, at 17 px with an absolute 1.7 px stroke (`absoluteStrokeWidth`), which is the design's weight. Semantic names stay the FigCraft names (`core`, `memory`, `dreams`, …), each mapped to one Lucide icon in a single table. In FigCraft, each `Icon/*` component's vector is replaced by the real Lucide SVG (same component, same id, so every instance stays linked), and the I2 glyphs are added the same way. The design and the code then draw identical icons.

| Design name | Lucide | Design name | Lucide | Design name | Lucide |
|---|---|---|---|---|---|
| addons | `puzzle` | file | `file` | refresh | `rotate-cw` ⚑ |
| agent | `bot` | history | `history` | restore | `rotate-ccw` |
| alert | `triangle-alert` | home | `house` | save | `save` |
| attach | `paperclip` | info | `info` | search | `search` |
| channels | `hash` | key | `key` | send | `arrow-up` ⚑ |
| chat | `message-square` | lock | `lock` | settings | `settings` |
| check | `check` | logout | `log-out` | shield | `shield` |
| chevron-down/left/right | `chevron-*` | mcp | `plug` | sidebar-collapse | `panel-left-close` |
| close | `x` | memory | `book` | skills | `wand-sparkles` |
| cloud | `cloud` | mic | `mic` | tasks | `circle-check-big` |
| copy | `copy` | more | `ellipsis` | theme | `contrast` |
| core | `circle-dot` | pencil | `pencil` | trash | `trash-2` |
| download | `arrow-down-to-line` ⚑ | plus | `plus` | unlink | `unlink` |
| dreams | `moon` | | | upload | `arrow-up-to-line` ⚑ |
| external | `external-link` | | | usage | `trending-up` |
| | | | | user / users | `user` / `users` |

⚑ marks where the drawn icon differs from Lucide's same-named icon, and the closer Lucide icon was picked: `send` is drawn as an up arrow (Lucide `send` is a paper plane); `refresh` is one arc; `download`/`upload` have a baseline. `addons` (puzzle) and `key` are simplified in the drawing; Lucide's versions replace them.

## B. Brand

| ID | Component | Source | Variants / states | Appears on | Replaces |
|----|-----------|--------|-------------------|------------|----------|
| B1 | Mark (8×8 dot V) | Brand page | State Idle/Live × Theme; Grid on/off | Brand, Login, Onboarding | — |
| B2 | Wordmark (star-dot) | Brand page | State Idle/Live × Theme | Brand, Login, Onboarding, Mobile Home | — |
| B3 | Sigil (5×5 agent identicon, a.k.a. agent seal) | Design System | Size Md 34 / Sm 22. **Screens also use** Lg ~64 (New agent preview) and Xl ~86 (Config identity) | everywhere | `avatar.tsx` (agent part) |
| B4 | LED board (mark drawn as 2×2 dot cells on a dot-grid field, with live dots) | **extracted** | live / idle | Login, Onboarding | `routes/login.tsx` art |

⚑ B3: FigCraft has hand-drawn sigils. Code must generate a symmetric 5×5 sigil from the agent id ("drawn from the agent ID, so it stays the same everywhere", New agent preview). The rule (hash → lit cells) is not in the design and gets defined during the build.

---

## D. Design System components (built as drawn, plus the screen extensions)

Columns: **Variants** are the FigCraft axes. **Screen extensions** are what the hand-drawn copies on screens add, and the library covers them too.

### Shell & navigation

| ID | Component | T | Variants | Screen extensions | Appears on | Replaces |
|----|-----------|---|----------|-------------------|------------|----------|
| D1 | Sidebar | T3 | Active (14 pages + None) × Collapsed | collapsed-rail tooltip label (Chat collapsed) | 18 pages | layout in `root.tsx` |
| D2 | NavItem | T1 | State Default/Active/Hover/Focus × Collapsed; props Icon, Label, Live | — | via Sidebar | — |
| D3 | NavSection | T1 | Collapsed | — | via Sidebar | — |
| D4 | AgentSwitcher | T2 | Collapsed × State Default/Hover/Open | **open state is the agent Picker** (X41): search with count, Recent, All A–Z, Owner badge, ✓ current, "42 more · type to filter", Create agent | via Sidebar, Scaling | — |
| D5 | UserMenu | T2 | Collapsed × State Default/Hover | — | via Sidebar | — |
| D6 | SettingsNav | T3 | Active (6) | — | 5 Settings pages | `settingsRoot.tsx` nav |
| D7 | PageHeader | T2 | — | mono meta line; right slot holding buttons, Segment range + SortSelect (Usage), Stepper (New agent), UnsavedIndicator + buttons (Core); **Settings size** (15 px title + description, Settings pages) | every app page | — |
| D8 | Tab | T1 | State Default/Selected/Danger/Hover/Focus/Disabled | — (Config tabs, incl. red Danger Zone) | Config | — |
| D9 | Step | T1 | State Done/Current/Upcoming/Error | used in the Stepper (X10) | New agent, Onboarding | — |

### Actions & inputs

| ID | Component | T | Variants | Screen extensions | Appears on | Replaces |
|----|-----------|---|----------|-------------------|------------|----------|
| D10 | Button | T1 | Variant Primary/Secondary/Danger/Destructive/Ghost × Size Md/Sm × State (6 incl. Loading) | **Lg / full width** (Sign in, Create account); **keyboard hint** inside ("Save Ctrl S"); **trailing icon** ("Continue to review ›"); dashed "Show all" variant (Home) | everywhere | — |
| D11 | IconButton | T1 | Variant Secondary/Ghost × State (6 incl. Selected) | **vertical group** (graph zoom in/out/fit, Memory); Sm close × in panels | Memory, Chat, Tasks | — |
| D12 | TextField | T1 | State Default/Placeholder/Focus/Error/Disabled/Hover/Read-only | **mono value**; **suffix unit** ("tokens", "%"); **locked** read-only with lock icon (Agent ID); password mask; **Lg** (Login 46 px); emerald focus | Config, New agent, Profile, Users, Login, Onboarding, MCP, Skill editor | — |
| D13 | Textarea | T1 | State (6) | **mono** (MCP arguments, Shell environment) | Config, New agent, MCP, Skill editor | — |
| D14 | Select | T2 | State (7 incl. Open) | **leading provider logo** (model select); mono value; **compact badge value** (role select in Users table) | Config, New agent, Users | `CustomSelect.tsx`, `ModelSelect.tsx` |
| D15 | SearchField | T1 | State Empty/Hover/Focus/Filled/Disabled | **count suffix** ("48", "38 of 1,024"); used as the Picker header | Home, Tasks, Skills, MCP, Users, Roles, Memory, Providers, Addons, Scaling | — |
| D16 | Segment | T1 | Selected × State; props Count | — | via FilterTabs | — |
| D17 | FilterTabs | T2 | — | **mode switch without counts**: Rich/Markdown/Split, Local/Docker, Remote URL/Local command, 7 days/30 days/Custom, Table/Raw, Largest/Recently changed/Most linked, Most active/Mine/Live/Failed | Home, Tasks, Skills, MCP, Users, Roles, Usage, Memory, New agent, Skill editor, Chat file preview | — |
| D18 | Switch | T1 | Checked × State | used in ToggleChip (X15) and SwitchRow (X16) | Tasks, MCP, Channels, Addons, New agent, Chat composer | — |
| D19 | Checkbox | T1 | Checked × State | used in PermissionRow (X57) | Roles | — |
| D20 | Radio | T1 | Selected × State | not drawn on any screen; kept as designed | — | — |
| D21 | Menu | T2 | — | search past ~8 items | — | — |
| D22 | MenuItem | T1 | Tone Default/Danger × State | — | — | — |

### Feedback & status

| ID | Component | T | Variants | Screen extensions | Appears on | Replaces |
|----|-----------|---|----------|-------------------|------------|----------|
| D23 | Badge | T1 | Tone Neutral/Rank/Live/Failed/Retrying | used for tags and keywords (`vector-db`, `review`), `built-in`, `custom`, `granted`, `Complete`, `agent · v1.2.0` | most pages | — |
| D24 | Status | T1 | State Live/Idle/Failed/Retrying/Starting/Stopped | ⚑ screens use more dot shapes than the 6 states: task runs (Answered ●, Running ◉, No response ○ amber, Not run yet ○ faint, Interrupted ◌), MCP (Connected, Reconnecting ✺ amber, Disabled ○), integrations (Reading ◉, Failed ●, Installed ●) | Home, Tasks, MCP, Channels, Addons, Chat | — |
| D25 | Notice | T2 | Tone Failed/Retrying/Info | **several actions** ("Retry · Switch model"); **compact** (inside the trail, mono message: "memory_write failed"); **limit** (amber, "Stopped at the 30 s script limit") | Chat states C1–C5, I4, I6; Channels | — |
| D26 | Banner | T2 | Tone Offline/Reconnected/Failed | **with actions** (agent-failed banner: Retry now, Open config); **inline in chat** (C6 "Connection lost · reconnecting in 3 s · Retry now") | States, Chat | — |
| D27 | Toast | T2 | Tone Success/Error/Warning/Info | — | States | `Toast.tsx` |
| D28 | Dialog | T2 | Tone Default/Danger | **type to confirm** (Delete Scribe); **large** content dialog (file preview, X37) | States, Chat | `providers/DeleteWarning.tsx` |
| D29 | EmptyState | T2 | Kind Empty/No results/Error | — | States | — |
| D30 | ProgressBar | T1 | State Running/Indeterminate/Done/Failed | — | States | — |
| D31 | Skeleton | T1 | (card) | **text-lines** variant (history loading, States loading) | Chat G2, States | `Skeleton.tsx` |
| D32 | DotLoader | T1 | (Tone) | — | Chat, States | `dot_loader.tsx` |
| D33 | Tooltip | T1 | (ink, flips in light) | **rich two-line** (context ring: "156k of 200k tokens / model · checkpoints automatically") | Chat collapsed, Chat E4 | `TooltipLabel.tsx` |

### Surfaces & data

| ID | Component | T | Variants | Screen extensions | Appears on | Replaces |
|----|-----------|---|----------|-------------------|------------|----------|
| D34 | Card | T2 | (Content slot) | **header description**; **header trailing slot** (Switch, count, close ×); **Danger tone** (red border, Danger zone); **dashed** look (hint, see X5) | almost every page | — |
| D35 | SlideOver | T2 | (Content slot) | — | — | `SlideOver.tsx` |
| D36 | Table | T2 | (composition) | **toggle column** (Tasks, MCP); **two-line cells** (title + description); **dense mono** data table (CSV preview); **action column** (API keys, Users) | Tasks, MCP, API keys, Users, Memory bundles, Chat file preview | — |
| D37 | TableHeader | T1 | — | mono uppercase labels | as Table | — |
| D38 | TableRow | T1 | State Default/Hover/Selected/Focus | — | as Table | — |
| D39 | Pagination | T2 | Position First/Middle/Last/Single | **compact** (summary + ‹ › only: Memory bundles, Roles) | Tasks, Skills, MCP, Users, Roles, Memory | — |
| D40 | Avatar | T1 | Size Sm/Md | **Lg** (Profile header, 56 px) | Settings, Config sharing, Users | `avatar.tsx` (person part) |
| D41 | CodeBlock | T2 | State Default/Copied | **line numbers** (run detail); **filename label** ("SKILL.md"); **diff** language (Skill editor example) | Chat, Skills, Skill editor | — |
| D42 | InlineCode | T1 | — | — | Chat replies | — |

### Chat

| ID | Component | T | Variants | Screen extensions | Appears on | Replaces |
|----|-----------|---|----------|-------------------|------------|----------|
| D43 | UserMessage | T3 | — | **with SkillChip**; **with attachments**; **Queued** state (dashed outline + "Queued · sends when … finishes") | Chat A3, B1, B2, Mobile Chat | `chat_bubble.tsx`, `MessageItem.tsx` |
| D44 | AgentHeader | T3 | — | — | Chat (every reply) | `MessageItem.tsx` |
| D45 | AgentReply | T3 | State Default/Hover/Streaming/Failed | **meta row** (reactions, add reaction, copy, model · in · out · cost); prose tables and code inside (see X49 Prose) | Chat, Mobile Chat | `MessageItem.tsx` |
| D46 | Composer | T3 | State Idle/Busy/Focus/Filled/Recording/Disabled | **attachments row**; **skills menu open** (X33); **recorded playback** + "Ask for a spoken reply" switch (X35); **ContextRing** beside Send (X36); custom disabled placeholder ("Archivist can't answer until it restarts"); Mobile token text ("21k / 200k") | Chat, Mobile Chat | chat route |
| D47 | Thinking | T3 | — | "Archivist is answering Scout's report…" wording | Chat | `ThinkingIndicator.tsx` |
| D48 | CheckpointDivider | T3 | — | **no-handover** variant ("/lobotomy"); opens HandoverSummary (X31) | Chat B3 | `CheckpointDivider.tsx` |
| D49 | ActivityTrail | T3 | (composition) | **collapsed summary** row with › (A5); **Mobile mini-timeline** summary | Chat A4/A5, C4, D1, I1–I8, Mobile Chat | `ActivityTrail.tsx` |
| D50 | TrailHead | T3 | State Live/Done/Failed | "stopped" count (I6) | as ActivityTrail | — |
| D51 | TrailCall | T3 | State Done/Running/Failed/Limit | "running…" in place of the duration; **lookup** look (quiet, no duration: list/describe_tool_function, I2) | as ActivityTrail | — |
| D52 | ScriptCall | T3 | State Ok/Failed | `×4` repeat count; **expandable arguments** block (I8) | Chat I3–I8 | — |
| D53 | Thought | T3 | — | — | Chat I1, I7 | — |
| D54 | Fold | T3 | — | — | Chat I7 | — |
| D55 | Reaction | T2 | State Default/Hover/Mine | **add-reaction** button (smile) beside it | Chat | `ReactionBadges.tsx` |
| D56 | SkillChip | T2 | — | — | Chat A3, B2 | — |
| D57 | Attachment | T2 | State Ready/Uploading/Failed | — | Chat, Chat E1 | `AttachmentChip.tsx` |
| D58 | CallChip | T2 | Status (8) | — | Chat F2 | `client/ClientCallChip.tsx` |
| D59 | BackgroundJob | T3 | State Running/Reporting/Reported/Undelivered/Cancelled/Interrupted | **pieces progress** dots ("2 of 3 pieces done"); "Jump to start · Cancel" actions; "lost" amber marker | Chat side panel, Chat D1, D3 | `BackgroundJobTray.tsx` |
| D60 | JobsPill | T2 | State Default/Hover/Focus | Mobile icon + count form | Chat, Mobile Chat | `BackgroundJobTray.tsx` |

---

## X. Extracted from screens (not on the Design System page)

### Layout & structure

| ID | Component | T | Variants / states (from all occurrences) | Appears on | Replaces |
|----|-----------|---|------------------------------------------|------------|----------|
| X1 | Eyebrow (section label) | T1 | plain / with count ("CONNECTED · 5", "PERMISSIONS · 4 OF 12", "ALL · A–Z") | Providers, Roles, Memory picker, Chat skill menu, Skills detail, Addons, Skill editor outline, Scaling, Dreams entries | — |
| X2 | PanelHeader (title + right mono meta) | T1 | with count / with close / with action | Chat side panel (Recalled memory, Background jobs, Session files), Core History, Memory Bundles, Usage cards | — |
| X3 | KeyValueList | T1 | label/value rows; value may hold Status, link or action ("Replace") | Tasks detail, Dreams status, Channels card, New agent preview | — |
| X4 | ActionBar (sticky bar: left/right action groups) | T2 | bottom (wizard Back / Continue) / top (editor: breadcrumb … unsaved + Discard + Save) | New agent, Skill editor | — |
| X5 | Callout (dashed hint) | T1 | text / with action | Home first run ("No API key?…"), Addons ("That's every addon…") | — |
| X6 | Kbd (key hint) | T1 | inline / inside a button | Skill editor (Ctrl S), Chat skill menu, file preview footer, topic picker (Enter) | `MediaKeyHint.tsx` |
| X7 | TextLink (text-only action) | T1 | Default / Danger; trailing → or ↗; Hover/Focus/Disabled | Chat notices (Retry, Switch model, Open Providers), jobs (Jump to start, Cancel), reports (Open … conversation →), Roles (See them in Users →), Tasks (Open run conversation ↗), Addons (Replace), MCP (+ Add variable), Skill editor (Delete skill) | — |
| X8 | UnsavedIndicator | T1 | "Unsaved changes" / "Unsaved · draft kept in this browser" | Core, Skill editor | — |
| X9 | Breadcrumb | T2 | path segments (mono) / back link + badge | Memory, Skill editor | — |
| X10 | Stepper | T2 | built from Step: header form (New agent: ✓ Config — ✓ System Prompt — 3 Tools — 4 Review) and compact form (Onboarding: 1 Account — 2 Provider — 3 First agent) | New agent, Onboarding | `routes/agent-new.tsx`, `routes/onboarding.tsx` |
| X11 | StatGroup / StatTile | T2 | label, value, sub-line; Failed tone value | Usage summary strip (5 tiles) | — |

### Inputs & forms

| ID | Component | T | Variants / states | Appears on | Replaces |
|----|-----------|---|-------------------|------------|----------|
| X12 | SortSelect | T2 | ⚑ three looks today: bordered "Sort **Next run** ⌄" (Tasks, Skills, MCP, Users); "Break down by **Activity** ⌄" (Usage); text-only "Sorted by documents ⌄" (Memory bundles, Roles). Proposal: one component, two looks (bordered / text) | Tasks, Skills, MCP, Users, Usage, Memory, Roles | — |
| X13 | InlineSelect | T1 | text + ▾ ("Vision model: Anthropic · claude-haiku-5-5 ▾") | New agent | — |
| X14 | SecretField | T2 | inline label prefix + masked mono value + Replace; or masked value + Replace in a KeyValueList | Addons, Channels | — |
| X15 | ToggleChip (bordered label + Switch) | T2 | on / off | New agent "Always on" | — |
| X16 | SwitchRow (label, optional badge, helper, Switch) | T2 | on / off / disabled-with-helper; badge "granted" | Channels, Addons, MCP (self-management), Chat E3 ("Ask for a spoken reply") | `SelfManagementSection.tsx` |
| X17 | ToggleSection (card with header Switch; body shows when on) | T2 | on (expanded) / off (collapsed) | New agent (Text-to-Speech, Speech-to-Text, Read Image, Image Generation, Shell) | `routes/agent-new.tsx`, `routes/agent-settings.tsx` |
| X18 | KeywordInput (tag chips with × + dashed "+ Add") | T2 | empty / filled / adding | Skill editor | — |
| X19 | DropZone | T2 | inline ("Drop files here, or browse") / overlay ("Drop to attach", full column) | Skill editor, Chat E2 | — |
| X20 | KeyValueEditor (env rows: key + secret value, + Add variable) | T2 | empty / rows | MCP add server | — |
| X21 | ShareList (person rows: avatar, name, permission, remove; add row: input + Share) | T3 | owner row (badge, no remove) / member row | Config | — |
| X22 | SigilPicker (large sigil + "Sigil · or upload") | T3 | generated / uploaded | Config | `AvatarCropModal.tsx` (upload part) |
| X23 | SecretReveal ("Key created. Copy it now" + read-only mono value + Copy) | T2 | — | API keys | — |

### Lists & rows

The screens draw many two-line list rows that share one anatomy: leading slot, title, subtitle, trailing meta/actions, and Hover/Selected/Focus states. Proposal: one **ListItem** primitive (X24), with the domain rows below built on it.

| ID | Component | T | Variants / states | Appears on | Replaces |
|----|-----------|---|-------------------|------------|----------|
| X24 | ListItem (primitive) | T1 | leading (icon / sigil / avatar / logo / dot) · title (sans or mono) · subtitle · trailing (meta / badge / buttons / chevron / ✓) × Default/Hover/Selected/Focus | (all rows below) | — |
| X25 | FileRow | T2 | icon, name, size; in a list or as a drop result | Chat session files, Skills resources, Skill editor resources | — |
| X26 | ActivityItem (actor, time, text; failed tone) | T2 | with sigil (Home "Since you were away") / plain (Integrations Activity) | Home, Channels, Addons | — |
| X27 | RevisionRow (rev N, who · source, time, +n −m) | T2 | Default / Selected | Core History | `VersionHistory.tsx` |
| X28 | RunRow (time, run Status, duration) | T2 | Default / Selected | Tasks detail | — |
| X29 | RecalledMemoryRow (dot sized by score, path, bundle, score) | T2 | — | Chat side panel | — |
| X30 | AgentRow (Mobile: sigil, name, Status, chevron) | T2 | live / idle / failed | Mobile Home | — |

### Chat (extracted)

| ID | Component | T | Variants / states | Appears on | Replaces |
|----|-----------|---|-------------------|------------|----------|
| X31 | HandoverSummary (checkpoint expanded) | T3 | — | Chat B3 | `CheckpointDivider.tsx` |
| X32 | StoppedNote ("■ Stopped by you at 14:03 · 1 queued message dropped") | T3 | — | Chat B2 | — |
| X33 | SkillMenu (slash menu: Eyebrow, command rows mono + description, key-hint footer) | T3 | filtering / no match | Chat A2 | — |
| X34 | ChatGreeting (empty topic: sigil, "What shall we deliberate?", meta) | T3 | — | Chat A1 | — |
| X35 | VoiceRecorder / VoicePlayback (Composer modes: ● timer waveform Stop; ▶ track time Discard) | T3 | recording / recorded | Chat E3 | — |
| X36 | ContextRing (0–100 % ring + rich Tooltip) | T2 | idle / filling / hover | Chat, Chat E4 | — |
| X37 | FilePreview (header: type tag, name, meta, Download, Open in new tab, ×; Table/Raw switch; body; footer with prev/next and file switcher, key hints) | T3 | dialog (Chat · file preview) / inline panel (H4); table / text / image body | Chat, Chat H4 | `AttachmentPreviewModal.tsx` |
| X38 | FilePeek (file card in a reply: type tag, name, meta, Open + preview of the first rows or lines) | T3 | table / markdown / pdf meta / no preview (Download) / failed (Retry, tinted) | Chat H2, H5 | — |
| X39 | FileTypeTag (CSV, MD, PDF, PNG, XLSX, ZIP) | T1 | — | Chat H2–H5, file preview | — |
| X40 | ImageGallery (thumbnails + "name · size" captions) | T3 | one / several | Chat H1 | — |
| X41 | Picker (searchable popover: SearchField with count, Eyebrow sections, rows with leading item, highlighted match, trailing meta/badge/✓, "N more · type to filter", footer actions) | T2 | ⚑ one primitive behind five drawn pickers: Agent (Scaling, D4), Conversation (Scaling: filter tabs with counts, day groups, channel icons, live dot), Topic (Chat G1: new-topic row with Enter), Bundle (Memory: Recent / All matches, New bundle · All bundles →), and the trigger buttons themselves (bundle "infra 42 docs ⌄", topic title ⌄) | Memory, Chat G1, Scaling | `ProviderModelPicker.tsx` |
| X42 | TopicHeader (topic name ⌄ + mono meta; opens the Topic Picker) | T3 | — | Chat, Mobile Chat | — |
| X43 | JumpToLatest ("3 new messages ↓") | T2 | — | Chat G3 | — |
| X44 | BackgroundReportCard (dashed: sigil, "Background job reported", meta, text, Open … →) | T3 | — | Chat, Chat D2 | `BackgroundReportItem.tsx` |
| X45 | AudioPlayer (▶, waveform progress, time / total, 1× speed, Show transcript) | T3 | paused / playing | Chat H3 | `VoiceMessagePlayer.tsx` |
| X46 | VideoPlayer (poster + ▶, caption) | T3 | — | Chat H3 | — |
| X47 | InteractivePage frame (title, "sandboxed page", fullscreen; body is the iframe) | T3 | — | Chat F1 | `client/InteractivePage.tsx` |
| X48 | RunDetail (expanded python run: description, CODE · N LINES + Copy, numbered code, CALLS INSIDE with expandable arguments, OUTPUT, RESULT + Copy) and Traceback (red tinted block, error line, "Show full traceback") | T3 | ok / raised / limit | Chat I5, I8 | `ExecutionReportView.tsx` |

### Rich text & editor

| ID | Component | T | Variants / states | Appears on | Replaces |
|----|-----------|---|-------------------|------------|----------|
| X49 | Prose (rendered markdown: h1–h3, paragraph, lists, table, code block, blockquote, inline code) | T4 | in a reply (body size) / in the editor (document size) | Chat replies, Core, Skill editor, Chat H2 | `MarkdownEditor.tsx` (view part), react-markdown styles |
| X50 | EditorToolbar | T4 | compact (Core: Paragraph, B I list code link, char count) / full (Skill editor: block-type select, B I code link, bullets, numbers, outline, table, / Insert block, Rich/Markdown/Split, fullscreen) | Core, Skill editor | `MarkdownEditor.tsx` |
| X51 | OutlineNav (OUTLINE, nested headings by level, active item) | T4 | — | Skill editor | — |
| X52 | DiffView (+ / − / context lines with tinted rows, Restore) | T4 | — | Core History | `VersionHistory.tsx` |

### Agents, home & dreams

| ID | Component | T | Variants / states | Appears on | Replaces |
|----|-----------|---|-------------------|------------|----------|
| X53 | AgentCard, the Home court card (sigil, name, Owner badge, description, Status, model) | T3 | live / idle / failed (retry) × owned or not | Home | `AgentCard.tsx` |
| X54 | SetupStepCard (✓ done / numbered current / faint upcoming; title, description, trailing action) | T3 | done / current / upcoming | Home first run | `SetupProgress.tsx` |
| X55 | AgentPreviewCard (PREVIEW, sigil Lg, name, id · description, KeyValueList, note) | T3 | — | New agent | — |
| X56 | DreamJournalEntry, the dream activity card (time range, Complete badge, meta; Extraction / Consolidation stats; change lines with kind wrote / revised / core) | T3 | complete / (running, failed: not drawn) | Dreams | `routes/agent-dream.tsx` |
| X57 | PermissionRow + PermissionGroup (Checkbox, label, mono key; group heading) | T3 | granted / not / disabled (built-in role) | Roles | `RolesSection.tsx` |
| X58 | AgentStartingCard (dot loader, "Starting Treasurer…", detail, ProgressBar, ×) | T3 | pulling / building | States | — |

### Integrations & settings

| ID | Component | T | Variants / states | Appears on | Replaces |
|----|-----------|---|-------------------|------------|----------|
| X59 | IntegrationCard (logo, name, handle, Status + Switch, KeyValueList or settings, ToolList, failed Notice inside, actions) | T3 | reading / failed / installed / disabled | Channels, Addons | `integrations.tsx` |
| X60 | IntegrationLogo (Discord, Telegram, Brave tiles) | T1 | sizes | Channels, Addons | — |
| X61 | ToolList (mono tool name + description) | T2 | — | Addons | — |
| X62 | ServerRow (MCP: remote/local icon, name, url, Status with detail, category, tool count, Switch) | T3 | connected / reconnecting / disabled | MCP | `routes/integrations-mcp.tsx` |
| X63 | ProviderLogo (letter tile in the provider's brand colour) | T1 | sizes Sm (selects) / Md (rows, tiles) | Providers, Config, New agent | `ProviderLogo.tsx` |
| X64 | ProviderRow (logo, name, saved key / url meta, usage, Models, ⋯) | T3 | connected / custom badge / not used yet | Providers | `providers/ConnectedList.tsx` |
| X65 | ProviderTile (logo, name, sub-line) | T3 | Default / Hover / Focus | Providers "Add a provider" | `providers/AddProvider.tsx` |
| X66 | ProfileHeader (Avatar Lg, name, role badge) | T3 | — | Profile | — |
| X67 | RoleRow (mono name, built-in badge, permission count, people count) | T2 | Default / Selected | Roles | `RolesSection.tsx` |

### Visualisation

| ID | Component | T | Variants / states | Appears on | Replaces |
|----|-----------|---|-------------------|------------|----------|
| X68 | ActivityGrid, the Home agent activity card ("Last 24 hours": rows × 24 hours, dot size = turns, live dot) | T4 | no agents (axis + empty message + action) / one agent (single row + sentence) / 2–6 rows / 1,000 agents ("All N" row + top 5 + Most active · Mine · Live · Failed switch + "995 more in Usage →") / Mobile compact | Home, Home · activity at any size, Mobile Home | `routes/home.tsx` |
| X69 | NightStrip, the dream timeline ("Last 30 nights": dot size = memories, ring = CORE changed) | T4 | — | Dreams | `routes/agent-dream.tsx` |
| X70 | StackedBarChart (daily cost by activity, legend, dated annotation marker) | T4 | — | Usage | `UsageDailyChart.tsx` |
| X71 | BarList / Meter (label, bar, value; "Other · N" row) | T4 | share column in a table (Usage "Where it went") / standalone list (Scaling "Cost by model") | Usage, Scaling | — |
| X72 | Legend (swatch + label) | T1 | — | Usage | — |
| X73 | MemoryGraph (stars sized by links, faded 2 hops, dashed edge to another bundle, selected ring, labels; legend caption; IconButton group) | T4 | document neighbourhood / bundle graph (Largest · Recently changed · Most linked) | Memory, Memory · all bundles | `MemoryGraph.tsx` |
| X74 | DocumentDetail (title, path meta, tags, summary, LINKS TO / LINKED FROM, provenance, Edit · History) and SelectedBundle (name, counts, Open bundle · Export .zip · Delete) | T3 | document / bundle | Memory, Memory · all bundles | `routes/memory.tsx` |

### Mobile

| ID | Component | T | Variants / states | Appears on | Replaces |
|----|-----------|---|-------------------|------------|----------|
| X75 | MobileTopBar (☰, wordmark or sigil + topic + status, trailing action or jobs count) | T3 | home / chat | Mobile Home, Mobile Chat | — |

---

## Excluded (seen, not components)

- **Screen layouts**: the page frames themselves, the Settings two-column layout, the Chat three-column layout, the Login/Onboarding split panel and its capability lines, the Home "court" grid arrangement.
- **Design-board annotation**: Chat states lane headers, the `A1`…`I8` step pills, `on: …` trigger captions, the "Turn state machine" diagram, the Scaling "Rules" card list, the "Long names" explainer, the Home "at any size" explanatory text, and the Brand page's sections, explorations and "In use" mock-ups (terminal banner, social card, docs header).
- **Utilities, not components**: middle truncation of long names (Scaling) and number abbreviation. These are formatting helpers the components use, documented in the README rather than given catalogue entries.

## Token audit

- 18,561 solid fills on the screen pages; **162 are not bound to a variable** (0.9%).
- Expected: third-party brand colours on provider and integration logos (`#D97757`, `#5865F2`, `#2AABEE`, `#FB542B`, `#10A37F`, `#4285F4`, `#6566F1`, `#24292F`, `#FA520F`, `#4D6BFE`, …). Proposal: allowed **only** as data inside ProviderLogo / IntegrationLogo, never as tokens (an exception to FR-015).
- ⚑ Drift: `#A0A0AA` unbound in 46 places across 6 pages (API keys, Addons, Channels, Chat, Config, Core). It equals `text/muted` (dark) but won't flip in light. `#000000` ×45 (Addons, Chat, Memory, Usage) and `#FFFFFF` ×36 (Channels, Config, Providers, States): mostly glyphs on logo tiles and image placeholders, but worth a look. The library uses tokens either way. These are fixes to the FigCraft file, not to the library.

## WebUI features with no design (gaps)

These exist in today's WebUI but nowhere in the FigCraft file, so they are **not** in this library until they're designed: `DatePicker.tsx` (one-time task date), `EmojiPickerPopup.tsx` (what the add-reaction button opens), `providers/ModelsDrawer.tsx` (the "Models" button), `providers/ConnectForm.tsx` and `CustomProviderForm.tsx` (connecting a provider), `AvatarCropModal.tsx` (crop step of the sigil upload), `UsageConversationDrawer.tsx` (clicking a costliest conversation), `EmbeddingIndexerSection.tsx` (drawn only as a Config card section; covered by Card + Select).

## Totals

| Group | Count |
|-------|-------|
| Foundations | 6 groups (37 colour, 18 size, 7 tone modes, 24 type, 2 elevation, 2 fonts) |
| Icons | 47 + 25, all from Lucide |
| Brand | 4 |
| Design System components | 60 (38 carry screen extensions or usage notes) |
| Extracted from screens | 75 |
| **Components to build** | **139** (brand + D + X) |

## Review decisions (2026-10-23)

1. **Extracted components go back into FigCraft.** Every X component is added to the FigCraft Design System page as a component, through the FigCraft MCP, before it is built. Icons are the real Lucide SVGs (see *I. Icons*). FigCraft and the catalogue stay one-to-one, and each catalogue entry points at a Design System component.
2. **Merges accepted, where they fit.**
   - One **Picker** (X41) behind the agent, conversation, topic and bundle pickers.
   - One **SortSelect** (X12) with a bordered look and a text look.
   - One **ListItem** (X24) behind the domain rows (X25–X30, X64, X67).
   - A widened **Status** (D24) that covers the task-run, MCP and integration states.
   - Brand colours allowed **only** inside ProviderLogo and IntegrationLogo (X60, X63).

   "Where they fit" means: if a drawn variant can't be expressed by the merged component without a special-case prop, it stays a separate component, and the inventory row records why.
3. **Nothing is cut or deferred.** Everything here is needed for the WebUI rewrite, T4 included. Tiers set the build order only.
