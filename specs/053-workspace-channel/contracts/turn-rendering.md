# Contract: Turn Rendering

The WebUI's model for a turn (FR-034 to FR-039). Pure code in `webui/app/lib/trail.ts`, tested with vitest. The layout is [mockup.html](../mockup.html).

## Model

```ts
type TurnBlock =
  | { kind: 'trail'; events: TrailEvent[]; startedAt?: string; endedAt?: string }
  | { kind: 'message'; id: string; text: string; mentions?: MentionView[] }

type Turn = { /* existing fields, with `trail` replaced by: */ blocks: TurnBlock[] }
```

`groupHistory` produces blocks in entry order:

| Entry | Effect |
|---|---|
| `AssistantMessage` with text | A `message` block. The next event opens a new `trail` |
| `think`, `execute_python`, other tool calls | Appended to the open `trail`, opened if needed |
| `ToolResult` | Pairs by `call_id` with its event in **any** earlier block |
| `Response` | The turn's outcome, as today |

- A `trail` block is never empty, so text first means no trail above it.
- `narration` leaves `TrailEvent` and the `note` count leaves `trailSummary`.
- Reload and live use the same function, since the running trail already merges `history_entry` frames by uid.

## Drawing

| Part | Rule |
|---|---|
| Agent head | Once per turn. In a Workspace, again wherever another author's entry lands inside the turn (FR-038) |
| Trail head | `Worked {span}` with that block's counts. Span comes from the timestamps of its first and last entries |
| Open trail | The last block while the turn runs: live and expanded. All others folded |
| `message` block | `Markdown` at `type-prose`, the same as the reply. No stats, reactions or copy |
| Final message | `AgentReply`, as today. Stats carry the turn's total time |
| Failed or stopped | Earlier blocks stay. The last trail shows the failure, as today |

## Workspace additions

Pure, in `webui/app/lib/workspaces.ts`:

- Entries of a topic become an ordered list of people's messages and per-agent turn runs. A run ends when another author's entry lands, and continues under a repeated head if the same turn goes on. Turn identity is `activated_by_seq` plus agent.
- A `mention_agent` entry has no `history_uid`. It becomes a `message` block in its author's turn at its `seq`, and its python call pairs across the gap.
- The "woke" line comes from `mentions`, for `agent` targets.

## Design components

Changed in `design/` first, with stories, then used in `webui/`.

| Component | Change |
|---|---|
| `Rail` | New `workspaces` prop: initials tile, mention count, working dot, marker, its own `+`, separators |
| `UserMessage` | New `side="left"`, for other people, with mention highlight |
| `MentionChip` | New. Tones: resolved, you |
| `MentionMenu` | Items may be agents or people, with a sigil or avatar and status |
| `WakeNote` | New. "woke X", or a failed tone |
| `OriginBadge` | New, built on `Badge` |
| `MemberSearch` | New. Combobox over grouped results with a cap note, built like `Picker` |
| `ActivityTrail` | No change. A turn draws several |
