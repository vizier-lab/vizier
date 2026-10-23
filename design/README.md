# @vizier/design

Vizier's UI component library and its catalogue.

The catalogue (Storybook) is the **source of truth** for Vizier's design:
tokens, type, icons, brand pieces and components. The FigCraft "Celestial
Court" file was only the starting design; nothing in this repository reads or
writes it, and a design change is made here, in code, and reviewed in the
catalogue.

## Start

```sh
git clone https://github.com/vizier-lab/design.git && cd design
npm install
npm run storybook        # http://localhost:6006
```

Inside a vizier checkout the library lives at `design/` as a git submodule.

## Scripts

| Script | What it does |
|--------|--------------|
| `npm run storybook` | The catalogue on `http://localhost:6006` |
| `npm run build-storybook` | A static copy in `storybook-static/` (not hosted) |
| `npm run typecheck` | `tsc --noEmit` |
| `npm test` | Renders every story headless in Chromium, in both themes, with axe; fails on a critical accessibility violation, a render error, or an entry missing its inventory id. Lesser findings (contrast on `text/faint`, for one) show in each story's Accessibility panel |
| `npm run lint:tokens` | Fails on raw colours or sizes outside `src/tokens/`, and on padding, margin or gap off the space scale |
| `node scripts/inventory-checklist.mjs` | Regenerates the checklist below from the stories; `--check <inventory.md>` also fails on a missing or unknown id |

`npm test` needs Chromium once: `npx playwright install chromium` (on Linux, `sudo npx playwright install-deps chromium` for its system libraries).

## Using the library

The package is TypeScript source with no build step. Depend on it by path; inside vizier that is the `design/` submodule:

```json
"dependencies": { "@vizier/design": "file:../design" }
```

A `file:` dependency is a symlink, so the library's files would otherwise find React (and its types) in `design/node_modules` and the app would run two Reacts. Point both at the app's copy:

```ts
// vite.config.ts
export default defineConfig({ resolve: { dedupe: ["react", "react-dom"] } /* , plugins … */ });
```

```jsonc
// tsconfig.json, compilerOptions
"paths": {
  "react": ["./node_modules/@types/react"],
  "react/*": ["./node_modules/@types/react/*"],
  "react-dom": ["./node_modules/@types/react-dom"],
  "react-dom/*": ["./node_modules/@types/react-dom/*"]
}
```

Once, in the app's CSS entry, import the styles and point Tailwind at the library's sources so it generates their classes:

```css
@import "@vizier/design/styles.css";   /* fonts, tokens, type, Tailwind theme */
@source "../node_modules/@vizier/design/src";
```

Then import components, icons and brand pieces from the package root:

```tsx
import { Button, Notice, Icon, Sigil } from "@vizier/design";

<Button variant="primary" icon="plus">New agent</Button>
<Notice tone="failed" title="Scribe couldn't start">Model not found on Ollama.</Notice>
<Icon name="memory" tone="live" />
```

- **Theme** is CSS only: `<html data-theme="dark">` or `"light"`, or no attribute to follow the OS. There is no provider component.
- **Tokens alone**: `@import "@vizier/design/tokens.css"` gives just the `--vz-*` variables and `.tone-*` classes, for code that isn't on Tailwind.
- **Utilities** resolve only to tokens: `bg-surface`, `text-muted`, `border-border-strong`, `rounded-md`, `p-12`, `type-ui-medium`. Tailwind's default palette and font sizes are switched off.
- **Icons** use the design's names (`core`, `memory`, `dreams`, `send`), not Lucide's; `IconName` lists them, and `Icons/All` shows them.
- **Components never fetch**: content, collections and callbacks come in as props. Navigation takes an `href` or a callback; nothing knows about routers or stores.
- **Props** follow the design's variant names (`variant`, `size`, `tone`, `selected`). Hover, pressed and focus are CSS; app states (`disabled`, `loading`, `invalid`, `open`) are props. Native props pass through and `className` merges last.

## Reviewing

Every entry starts with the `needs-review` tag. To review one (say **Components/Feedback/Notice**):

1. The docs page header shows its inventory id and the WebUI file it will replace.
2. Switch the toolbar theme between Dark, Light and System; the render follows without a reload.
3. Open the **States** story: default, hover, pressed, focus and the app states, side by side.
4. Tab through the Default story: a 2px focus ring at a 2px offset; text fields focus emerald.
5. Check the **Accessibility** panel.

To approve, change `tags: ["needs-review"]` to `["approved"]` in the story file and commit; then rerun `node scripts/inventory-checklist.mjs`. To ask for changes, open an issue on [vizier-lab/design](https://github.com/vizier-lab/design/issues) that links the story's URL, and leave the tag as it is. Filter the sidebar by `needs-review` to see what is left. A component is approved only after the components it is built from.

## Inventory checklist

<!-- inventory:start -->

139 entries · 0 approved · 139 need review. Generated by `node scripts/inventory-checklist.mjs`; don't edit by hand.

| | Id | Catalogue entry | Status |
|---|----|-----------------|--------|
| ⬜ | B1 | `Brand/Mark` | needs-review |
| ⬜ | B2 | `Brand/Wordmark` | needs-review |
| ⬜ | B3 | `Brand/Sigil` | needs-review |
| ⬜ | B4 | `Brand/LedBoard` | needs-review |
| ⬜ | D1 | `Components/Shell/Sidebar` | needs-review |
| ⬜ | D2 | `Components/Shell/NavItem` | needs-review |
| ⬜ | D3 | `Components/Shell/NavSection` | needs-review |
| ⬜ | D4 | `Components/Shell/AgentSwitcher` | needs-review |
| ⬜ | D5 | `Components/Shell/UserMenu` | needs-review |
| ⬜ | D6 | `Components/Shell/SettingsNav` | needs-review |
| ⬜ | D7 | `Components/Shell/PageHeader` | needs-review |
| ⬜ | D8 | `Components/Shell/Tab` | needs-review |
| ⬜ | D9 | `Components/Shell/Step` | needs-review |
| ⬜ | D10 | `Components/Actions & inputs/Button` | needs-review |
| ⬜ | D11 | `Components/Actions & inputs/IconButton` | needs-review |
| ⬜ | D12 | `Components/Actions & inputs/TextField` | needs-review |
| ⬜ | D13 | `Components/Actions & inputs/Textarea` | needs-review |
| ⬜ | D14 | `Components/Actions & inputs/Select` | needs-review |
| ⬜ | D15 | `Components/Actions & inputs/SearchField` | needs-review |
| ⬜ | D16 | `Components/Actions & inputs/Segment` | needs-review |
| ⬜ | D17 | `Components/Actions & inputs/FilterTabs` | needs-review |
| ⬜ | D18 | `Components/Actions & inputs/Switch` | needs-review |
| ⬜ | D19 | `Components/Actions & inputs/Checkbox` | needs-review |
| ⬜ | D20 | `Components/Actions & inputs/Radio` | needs-review |
| ⬜ | D21 | `Components/Actions & inputs/Menu` | needs-review |
| ⬜ | D22 | `Components/Actions & inputs/MenuItem` | needs-review |
| ⬜ | D23 | `Components/Feedback/Badge` | needs-review |
| ⬜ | D24 | `Components/Feedback/Status` | needs-review |
| ⬜ | D25 | `Components/Feedback/Notice` | needs-review |
| ⬜ | D26 | `Components/Feedback/Banner` | needs-review |
| ⬜ | D27 | `Components/Feedback/Toast` | needs-review |
| ⬜ | D28 | `Components/Feedback/Dialog` | needs-review |
| ⬜ | D29 | `Components/Feedback/EmptyState` | needs-review |
| ⬜ | D30 | `Components/Feedback/ProgressBar` | needs-review |
| ⬜ | D31 | `Components/Feedback/Skeleton` | needs-review |
| ⬜ | D32 | `Components/Feedback/DotLoader` | needs-review |
| ⬜ | D33 | `Components/Feedback/Tooltip` | needs-review |
| ⬜ | D34 | `Components/Surfaces & data/Card` | needs-review |
| ⬜ | D35 | `Components/Surfaces & data/SlideOver` | needs-review |
| ⬜ | D36 | `Components/Surfaces & data/Table` | needs-review |
| ⬜ | D37 | `Components/Surfaces & data/TableHeader` | needs-review |
| ⬜ | D38 | `Components/Surfaces & data/TableRow` | needs-review |
| ⬜ | D39 | `Components/Surfaces & data/Pagination` | needs-review |
| ⬜ | D40 | `Components/Surfaces & data/Avatar` | needs-review |
| ⬜ | D41 | `Components/Surfaces & data/CodeBlock` | needs-review |
| ⬜ | D42 | `Components/Surfaces & data/InlineCode` | needs-review |
| ⬜ | D43 | `Components/Chat/UserMessage` | needs-review |
| ⬜ | D44 | `Components/Chat/AgentHeader` | needs-review |
| ⬜ | D45 | `Components/Chat/AgentReply` | needs-review |
| ⬜ | D46 | `Components/Chat/Composer` | needs-review |
| ⬜ | D47 | `Components/Chat/Thinking` | needs-review |
| ⬜ | D48 | `Components/Chat/CheckpointDivider` | needs-review |
| ⬜ | D49 | `Components/Chat/ActivityTrail` | needs-review |
| ⬜ | D50 | `Components/Chat/TrailHead` | needs-review |
| ⬜ | D51 | `Components/Chat/TrailCall` | needs-review |
| ⬜ | D52 | `Components/Chat/ScriptCall` | needs-review |
| ⬜ | D53 | `Components/Chat/Thought` | needs-review |
| ⬜ | D54 | `Components/Chat/Fold` | needs-review |
| ⬜ | D55 | `Components/Chat/Reaction` | needs-review |
| ⬜ | D56 | `Components/Chat/SkillChip` | needs-review |
| ⬜ | D57 | `Components/Chat/Attachment` | needs-review |
| ⬜ | D58 | `Components/Chat/CallChip` | needs-review |
| ⬜ | D59 | `Components/Chat/BackgroundJob` | needs-review |
| ⬜ | D60 | `Components/Chat/JobsPill` | needs-review |
| ⬜ | X1 | `Components/Layout/Eyebrow` | needs-review |
| ⬜ | X2 | `Components/Layout/PanelHeader` | needs-review |
| ⬜ | X3 | `Components/Layout/KeyValueList` | needs-review |
| ⬜ | X4 | `Components/Layout/ActionBar` | needs-review |
| ⬜ | X5 | `Components/Layout/Callout` | needs-review |
| ⬜ | X6 | `Components/Layout/Kbd` | needs-review |
| ⬜ | X7 | `Components/Layout/TextLink` | needs-review |
| ⬜ | X8 | `Components/Layout/UnsavedIndicator` | needs-review |
| ⬜ | X9 | `Components/Layout/Breadcrumb` | needs-review |
| ⬜ | X10 | `Components/Layout/Stepper` | needs-review |
| ⬜ | X11 | `Components/Layout/StatGroup` | needs-review |
| ⬜ | X12 | `Components/Actions & inputs/SortSelect` | needs-review |
| ⬜ | X13 | `Components/Actions & inputs/InlineSelect` | needs-review |
| ⬜ | X14 | `Components/Actions & inputs/SecretField` | needs-review |
| ⬜ | X15 | `Components/Actions & inputs/ToggleChip` | needs-review |
| ⬜ | X16 | `Components/Actions & inputs/SwitchRow` | needs-review |
| ⬜ | X17 | `Components/Actions & inputs/ToggleSection` | needs-review |
| ⬜ | X18 | `Components/Actions & inputs/KeywordInput` | needs-review |
| ⬜ | X19 | `Components/Actions & inputs/DropZone` | needs-review |
| ⬜ | X20 | `Components/Actions & inputs/KeyValueEditor` | needs-review |
| ⬜ | X21 | `Components/Actions & inputs/ShareList` | needs-review |
| ⬜ | X22 | `Components/Actions & inputs/SigilPicker` | needs-review |
| ⬜ | X23 | `Components/Actions & inputs/SecretReveal` | needs-review |
| ⬜ | X24 | `Components/Lists/ListItem` | needs-review |
| ⬜ | X25 | `Components/Lists/FileRow` | needs-review |
| ⬜ | X26 | `Components/Lists/ActivityItem` | needs-review |
| ⬜ | X27 | `Components/Lists/RevisionRow` | needs-review |
| ⬜ | X28 | `Components/Lists/RunRow` | needs-review |
| ⬜ | X29 | `Components/Lists/RecalledMemoryRow` | needs-review |
| ⬜ | X30 | `Components/Lists/AgentRow` | needs-review |
| ⬜ | X31 | `Components/Chat/HandoverSummary` | needs-review |
| ⬜ | X32 | `Components/Chat/StoppedNote` | needs-review |
| ⬜ | X33 | `Components/Chat/SkillMenu` | needs-review |
| ⬜ | X34 | `Components/Chat/ChatGreeting` | needs-review |
| ⬜ | X35 | `Components/Chat/VoiceRecorder` | needs-review |
| ⬜ | X36 | `Components/Chat/ContextRing` | needs-review |
| ⬜ | X37 | `Components/Chat/FilePreview` | needs-review |
| ⬜ | X38 | `Components/Chat/FilePeek` | needs-review |
| ⬜ | X39 | `Components/Chat/FileTypeTag` | needs-review |
| ⬜ | X40 | `Components/Chat/ImageGallery` | needs-review |
| ⬜ | X41 | `Components/Actions & inputs/Picker` | needs-review |
| ⬜ | X42 | `Components/Chat/TopicHeader` | needs-review |
| ⬜ | X43 | `Components/Chat/JumpToLatest` | needs-review |
| ⬜ | X44 | `Components/Chat/BackgroundReportCard` | needs-review |
| ⬜ | X45 | `Components/Chat/AudioPlayer` | needs-review |
| ⬜ | X46 | `Components/Chat/VideoPlayer` | needs-review |
| ⬜ | X47 | `Components/Chat/InteractivePage` | needs-review |
| ⬜ | X48 | `Components/Chat/RunDetail` | needs-review |
| ⬜ | X49 | `Components/Rich text/Prose` | needs-review |
| ⬜ | X50 | `Components/Rich text/EditorToolbar` | needs-review |
| ⬜ | X51 | `Components/Rich text/OutlineNav` | needs-review |
| ⬜ | X52 | `Components/Rich text/DiffView` | needs-review |
| ⬜ | X53 | `Components/Agents & dreams/AgentCard` | needs-review |
| ⬜ | X54 | `Components/Agents & dreams/SetupStepCard` | needs-review |
| ⬜ | X55 | `Components/Agents & dreams/AgentPreviewCard` | needs-review |
| ⬜ | X56 | `Components/Agents & dreams/DreamJournalEntry` | needs-review |
| ⬜ | X57 | `Components/Agents & dreams/PermissionRow` | needs-review |
| ⬜ | X58 | `Components/Agents & dreams/AgentStartingCard` | needs-review |
| ⬜ | X59 | `Components/Integrations & settings/IntegrationCard` | needs-review |
| ⬜ | X60 | `Components/Integrations & settings/IntegrationLogo` | needs-review |
| ⬜ | X61 | `Components/Integrations & settings/ToolList` | needs-review |
| ⬜ | X62 | `Components/Integrations & settings/ServerRow` | needs-review |
| ⬜ | X63 | `Components/Integrations & settings/ProviderLogo` | needs-review |
| ⬜ | X64 | `Components/Integrations & settings/ProviderRow` | needs-review |
| ⬜ | X65 | `Components/Integrations & settings/ProviderTile` | needs-review |
| ⬜ | X66 | `Components/Integrations & settings/ProfileHeader` | needs-review |
| ⬜ | X67 | `Components/Integrations & settings/RoleRow` | needs-review |
| ⬜ | X68 | `Components/Visualisation/ActivityGrid` | needs-review |
| ⬜ | X69 | `Components/Visualisation/NightStrip` | needs-review |
| ⬜ | X70 | `Components/Visualisation/StackedBarChart` | needs-review |
| ⬜ | X71 | `Components/Visualisation/BarList` | needs-review |
| ⬜ | X72 | `Components/Visualisation/Legend` | needs-review |
| ⬜ | X73 | `Components/Visualisation/MemoryGraph` | needs-review |
| ⬜ | X74 | `Components/Visualisation/DocumentDetail` | needs-review |
| ⬜ | X75 | `Components/Mobile/MobileTopBar` | needs-review |
| ✅ | I1, I2 | `Icons/All` (73 icons) | — |

<!-- inventory:end -->

## Licence

MIT. The bundled fonts (Schibsted Grotesk, JetBrains Mono) keep their own OFL
licences, carried by their Fontsource packages.
