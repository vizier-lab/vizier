# Contract: WebUI changes (mockup)

FigCraft was unreachable in this session (the `figcraft` MCP failed to connect), so per the
constitution the design is this ASCII mockup. **It is reviewed before any UI code is written**; if
FigCraft is available at implementation time, draw it on the usage page of `Vizier shell.figcraft`
first and keep this file in step. Order: `design/` (components + stories) → `webui/` → pin bump.

## Usage page (`webui/app/routes/usage.tsx`)

```text
┌ Usage ─────────────────────────────────────────────── [Last 30 days ▾] ┐
│ Activity  (All) Chat  Projects  Background  Scheduled  Dreams  …  Indexing │
│ Kind      (All) Text  Images  Speech  Transcription  Embeddings          │  ← new filter row
│                                                                         │
│  $61.70  ▲ 12% vs previous 30 days          18.4M tokens · 4,210 calls  │
│                                                                         │
│  ▁▂▃▅▃▂▆▇▅▃ … daily cost stacked by activity                           │
│   tooltip:  Nov 25 · $1.40                                              │
│             Chat $1.12 · Dreams $0.28                                   │
│             Text $1.12 · Images $0.25 · Embeddings $0.03   ← per kind   │
│                                                                         │
│  Breakdown  [Activity] [Kind] [Model] [Channel]           ← Kind is new │
│  ─────────────────────────────────────────────────────────────────────  │
│  Text            $47.20   76%   17.1M tokens        2,920 calls         │
│  Images           $9.80   16%   233 images            233 calls         │
│  Speech           $2.10    3%   140k chars · 2.5 h     640 calls        │
│  Transcription    $1.40    2%   3.1 h audio            188 calls        │
│  Embeddings       $1.20    2%   41M tokens            9,260 calls       │
│                                                                         │
│  Model breakdown row (media):                                           │
│  openai / gpt-image-1   Images   $9.80  16%  233 images   priced: table ⓘ│
│  whisper / base         Transcr.  free  —    22 min audio  local        │
│                                                                         │
│  Not in the estimate: 210k tokens, 12 images · custom/local-qwen, …     │
└─────────────────────────────────────────────────────────────────────────┘
```

- The Kind filter is the same chip control as the Activity filter; picking a kind hides the
  **Kind** breakdown tab and the headline adds "· 16% of all spend".
- A row's amount column shows the kind's own unit (`lib/usage.ts` formats: images, `140k chars`,
  `2.5 h audio`, tokens). A mixed row (Activity, Channel) shows tokens plus a second line of media
  units when it has any.
- `price_source = table` shows a small ⓘ: "Estimated from Vizier's price table (as of <date>)".
  `local` shows "free" and the word local; never "unpriced".
- Days before `media_accounting_since` fade when unfiltered or filtered to a media kind/Indexing.
- Conversation details list media calls among the calls: kind icon, model, variant, amount, cost.

## Chat step cost (Story 4, `screens/chat`, trail step details)

```text
▸ Generated an image                                         2.1 s
   ┌──────────────────────────────────────────────────────────┐
   │ … existing step details …                                │
   │ openai / gpt-image-1 · 1 image (1024×1024, high) · $0.17 │  ← new line per media call
   └──────────────────────────────────────────────────────────┘
```

Fetched from `GET /usage/calls?ids=…` when a step is expanded; nothing is drawn when the response
has no entry for the step. Uses the design's existing muted metadata text style.

## design/ components

- `UsageBreakdownRow` gains an optional `units` line.
- `FilterChips` (existing Activity chips) reused for Kind; no new component.
- `TrailStep` details gain an optional `meta` list slot (if it does not exist already).
