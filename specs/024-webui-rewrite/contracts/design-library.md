# Contract: Design library changes for the rewrite

The rewrite builds every screen from `@vizier/design`, following that library's contract (`specs/022-design-system-library/contracts/component-api.md`): props follow the variant axes, interaction states are CSS, no fetching, tokens only. When a screen needs something the library lacks, the change is made **in `design/` first**, with its story and inventory id, then used from the WebUI (spec FR-003). Every change below must pass the library's `npm run typecheck`, `npm test` (both themes + axe) and `npm run lint:tokens`, and gets a CHANGELOG line, since the WebUI now consumes the library (component-api "Stability").

## New components

### X76 MarkdownEditor

```ts
type MarkdownEditorProps = {
  value: string;                         // markdown
  onChange(markdown: string): void;
  mode?: "rich" | "markdown" | "split";  // default "rich"
  onModeChange?(mode): void;
  look?: "compact" | "full";             // Core: compact toolbar + char count; Skill editor: full + outline
  outline?: boolean;                     // shows OutlineNav from the document's headings (full look)
  placeholder?: string;
  readOnly?: boolean;
  onSaveShortcut?(): void;               // Ctrl/Cmd+S
} & Omit<HTMLAttributes<HTMLDivElement>, "onChange">;
```

- Composes EditorToolbar (X50), OutlineNav (X51) and Prose (`size="document"`).
- Markdown round-trips without loss for headings, lists, tables, code blocks, links, emphasis and blockquotes. A story pins the round-trip with a fixture document.
- Engine: Tiptap 3 (research R5). This is the library's only new runtime dependency, added under the library's Principle I review.
- Stories: compact (Core), full rich, markdown, split, read-only.

### X77 DateTimeField

`value: string | null` (ISO local date-time), `onChange`, `min?`, `invalid?`, `disabled?`. A TextField-styled trigger plus a calendar popover (month grid and time row) that you can drive with the keyboard (arrows, PgUp/PgDn, Enter, Esc). Used by one-time tasks.

### X78 EmojiPicker

`onSelect(emoji: string)`, `recent?: string[]`, `open`, `onOpenChange`, `trigger`. A searchable grid with a recent row, built on Picker's popover and search parts. The emoji data comes from a small bundled list (no network), sized to the reactions Discord and Telegram accept.

### X79 ImageCropper

`src: string`, `aspect?: number` (1), `onCrop(blob: Blob)`, `onCancel`. A square crop with drag and zoom (slider + wheel), in a Dialog. SigilPicker's "upload" opens it.

### X80 ConversationRow

`kind: "webui" | "discord" | "telegram"`, `title`, `place?`, `topics?`, `time`, `selected?`, `query?` (highlight). Built on ListItem. Used in the conversation Picker sections (Today / Yesterday / Older).

### X81 LoadMore

`onLoad()`, `loading?`, `label` ("Load older", "Show all 12"), `remaining?: number`, `look?: "button" | "auto"` (`auto` loads when scrolled into view, via an IntersectionObserver). Used by feeds, histories, side panels and the chat's upward history load.

## Changed components

| Component | Change | Compatibility |
|-----------|--------|---------------|
| NightStrip (X69) | `Night.live?`, `Night.hint?: string` (shown on hover/focus); `selected?: number`; `onSelectNight?(index)`. Each night is a `<button>` when `onSelectNight` is set | Additive. Without the new props it renders as today |
| ActivityGrid (X68) | `ActivityRow.hints?: string[]` (24 per-dot tooltips, e.g. "14:00 · 12 turns"); dots are focusable when hints are set | Additive |
| DreamJournalEntry (X56) | `DreamChange.kind` adds `"removed"`; `DreamChange.href?` | Additive |
| Picker (X41) | `onEndReached?()`, `loading?`, `search.mode?: "client" \| "server"` (default `client`, today's behaviour) | Additive |
| AgentStartingCard (X58) | `eta?: ReactNode` | Additive |

## Rules for the WebUI side

- The WebUI may hold screen layouts and compositions of library parts (forms, drawers, page grids) in `webui/app/`. It must not restyle a library component with arbitrary classes. Spacing and colour come from token utilities only, so the library's `lint:tokens` rule also runs over `webui/app` (the script takes a path).
- Brand colours appear only through ProviderLogo and IntegrationLogo.
- Markdown in replies goes through `react-markdown` inside `<Prose size="reply">`, and code blocks through the library's CodeBlock.
