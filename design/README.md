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
| `npm run build-storybook` | A static copy in `storybook-static/`; the hosted copy at [www.vizier.rs/design](https://www.vizier.rs/design) is built from it by the landing deploy |
| `npm run typecheck` | `tsc --noEmit` |
| `npm test` | Renders every story headless in Chromium, in both themes, with axe; fails on a critical accessibility violation, a render error, or an entry missing its inventory id. Lesser findings (contrast on `text/faint`, for one) show in each story's Accessibility panel |
| `npm run lint:tokens` | Fails on raw colours or sizes outside `src/tokens/`, and on padding, margin or gap off the space scale |
| `node scripts/inventory-checklist.mjs` | Regenerates the checklist below from the stories; `--check <inventory.md>` also fails on a missing or unknown id |

`npm test` needs Chromium once: `npx playwright install chromium` (on Linux, `sudo npx playwright install-deps chromium` for its system libraries).

## Hosting

The catalogue is published at [www.vizier.rs/design](https://www.vizier.rs/design).
A push to `master` that touches `src/`, `.storybook/` or the package files runs
`.github/workflows/deploy-storybook.yml`, which dispatches `vizier-lab/landing`
with the pushed commit; landing builds `storybook-static/` from it and copies the
result into the website's `dist/design/` before deploying Pages. A build failure
fails the website deploy, so a broken catalogue is never published.

The dispatch needs a `LANDING_DISPATCH_TOKEN` repository secret (a fine-grained
PAT with Contents read/write on `vizier-lab/landing`); without it the job warns
and skips. Running that workflow by hand republishes the current `master`.

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

139 entries · 139 approved · 0 need review. Generated by `node scripts/inventory-checklist.mjs`; don't edit by hand.

| | Id | Catalogue entry | Status |
|---|----|-----------------|--------|
| ✅ | B1 | `Brand/Mark` | approved |
| ✅ | B2 | `Brand/Wordmark` | approved |
| ✅ | B3 | `Brand/Sigil` | approved |
| ✅ | B4 | `Brand/LedBoard` | approved |
| ✅ | D1 | `Components/Shell/Sidebar` | approved |
| ✅ | D2 | `Components/Shell/NavItem` | approved |
| ✅ | D3 | `Components/Shell/NavSection` | approved |
| ✅ | D4 | `Components/Shell/AgentSwitcher` | approved |
| ✅ | D5 | `Components/Shell/UserMenu` | approved |
| ✅ | D6 | `Components/Shell/SettingsNav` | approved |
| ✅ | D7 | `Components/Shell/PageHeader` | approved |
| ✅ | D8 | `Components/Shell/Tab` | approved |
| ✅ | D9 | `Components/Shell/Step` | approved |
| ✅ | D10 | `Components/Actions & inputs/Button` | approved |
| ✅ | D11 | `Components/Actions & inputs/IconButton` | approved |
| ✅ | D12 | `Components/Actions & inputs/TextField` | approved |
| ✅ | D13 | `Components/Actions & inputs/Textarea` | approved |
| ✅ | D14 | `Components/Actions & inputs/Select` | approved |
| ✅ | D15 | `Components/Actions & inputs/SearchField` | approved |
| ✅ | D16 | `Components/Actions & inputs/Segment` | approved |
| ✅ | D17 | `Components/Actions & inputs/FilterTabs` | approved |
| ✅ | D18 | `Components/Actions & inputs/Switch` | approved |
| ✅ | D19 | `Components/Actions & inputs/Checkbox` | approved |
| ✅ | D20 | `Components/Actions & inputs/Radio` | approved |
| ✅ | D21 | `Components/Actions & inputs/Menu` | approved |
| ✅ | D22 | `Components/Actions & inputs/MenuItem` | approved |
| ✅ | D23 | `Components/Feedback/Badge` | approved |
| ✅ | D24 | `Components/Feedback/Status` | approved |
| ✅ | D25 | `Components/Feedback/Notice` | approved |
| ✅ | D26 | `Components/Feedback/Banner` | approved |
| ✅ | D27 | `Components/Feedback/Toast` | approved |
| ✅ | D28 | `Components/Feedback/Dialog` | approved |
| ✅ | D29 | `Components/Feedback/EmptyState` | approved |
| ✅ | D30 | `Components/Feedback/ProgressBar` | approved |
| ✅ | D31 | `Components/Feedback/Skeleton` | approved |
| ✅ | D32 | `Components/Feedback/DotLoader` | approved |
| ✅ | D33 | `Components/Feedback/Tooltip` | approved |
| ✅ | D34 | `Components/Surfaces & data/Card` | approved |
| ✅ | D35 | `Components/Surfaces & data/SlideOver` | approved |
| ✅ | D36 | `Components/Surfaces & data/Table` | approved |
| ✅ | D37 | `Components/Surfaces & data/TableHeader` | approved |
| ✅ | D38 | `Components/Surfaces & data/TableRow` | approved |
| ✅ | D39 | `Components/Surfaces & data/Pagination` | approved |
| ✅ | D40 | `Components/Surfaces & data/Avatar` | approved |
| ✅ | D41 | `Components/Surfaces & data/CodeBlock` | approved |
| ✅ | D42 | `Components/Surfaces & data/InlineCode` | approved |
| ✅ | D43 | `Components/Chat/UserMessage` | approved |
| ✅ | D44 | `Components/Chat/AgentHeader` | approved |
| ✅ | D45 | `Components/Chat/AgentReply` | approved |
| ✅ | D46 | `Components/Chat/Composer` | approved |
| ✅ | D47 | `Components/Chat/Thinking` | approved |
| ✅ | D48 | `Components/Chat/CheckpointDivider` | approved |
| ✅ | D49 | `Components/Chat/ActivityTrail` | approved |
| ✅ | D50 | `Components/Chat/TrailHead` | approved |
| ✅ | D51 | `Components/Chat/TrailCall` | approved |
| ✅ | D52 | `Components/Chat/ScriptCall` | approved |
| ✅ | D53 | `Components/Chat/Thought` | approved |
| ✅ | D54 | `Components/Chat/Fold` | approved |
| ✅ | D55 | `Components/Chat/Reaction` | approved |
| ✅ | D56 | `Components/Chat/SkillChip` | approved |
| ✅ | D57 | `Components/Chat/Attachment` | approved |
| ✅ | D58 | `Components/Chat/CallChip` | approved |
| ✅ | D59 | `Components/Chat/BackgroundJob` | approved |
| ✅ | D60 | `Components/Chat/JobsPill` | approved |
| ✅ | X1 | `Components/Layout/Eyebrow` | approved |
| ✅ | X2 | `Components/Layout/PanelHeader` | approved |
| ✅ | X3 | `Components/Layout/KeyValueList` | approved |
| ✅ | X4 | `Components/Layout/ActionBar` | approved |
| ✅ | X5 | `Components/Layout/Callout` | approved |
| ✅ | X6 | `Components/Layout/Kbd` | approved |
| ✅ | X7 | `Components/Layout/TextLink` | approved |
| ✅ | X8 | `Components/Layout/UnsavedIndicator` | approved |
| ✅ | X9 | `Components/Layout/Breadcrumb` | approved |
| ✅ | X10 | `Components/Layout/Stepper` | approved |
| ✅ | X11 | `Components/Layout/StatGroup` | approved |
| ✅ | X12 | `Components/Actions & inputs/SortSelect` | approved |
| ✅ | X13 | `Components/Actions & inputs/InlineSelect` | approved |
| ✅ | X14 | `Components/Actions & inputs/SecretField` | approved |
| ✅ | X15 | `Components/Actions & inputs/ToggleChip` | approved |
| ✅ | X16 | `Components/Actions & inputs/SwitchRow` | approved |
| ✅ | X17 | `Components/Actions & inputs/ToggleSection` | approved |
| ✅ | X18 | `Components/Actions & inputs/KeywordInput` | approved |
| ✅ | X19 | `Components/Actions & inputs/DropZone` | approved |
| ✅ | X20 | `Components/Actions & inputs/KeyValueEditor` | approved |
| ✅ | X21 | `Components/Actions & inputs/ShareList` | approved |
| ✅ | X22 | `Components/Actions & inputs/SigilPicker` | approved |
| ✅ | X23 | `Components/Actions & inputs/SecretReveal` | approved |
| ✅ | X24 | `Components/Lists/ListItem` | approved |
| ✅ | X25 | `Components/Lists/FileRow` | approved |
| ✅ | X26 | `Components/Lists/ActivityItem` | approved |
| ✅ | X27 | `Components/Lists/RevisionRow` | approved |
| ✅ | X28 | `Components/Lists/RunRow` | approved |
| ✅ | X29 | `Components/Lists/RecalledMemoryRow` | approved |
| ✅ | X30 | `Components/Lists/AgentRow` | approved |
| ✅ | X31 | `Components/Chat/HandoverSummary` | approved |
| ✅ | X32 | `Components/Chat/StoppedNote` | approved |
| ✅ | X33 | `Components/Chat/SkillMenu` | approved |
| ✅ | X34 | `Components/Chat/ChatGreeting` | approved |
| ✅ | X35 | `Components/Chat/VoiceRecorder` | approved |
| ✅ | X36 | `Components/Chat/ContextRing` | approved |
| ✅ | X37 | `Components/Chat/FilePreview` | approved |
| ✅ | X38 | `Components/Chat/FilePeek` | approved |
| ✅ | X39 | `Components/Chat/FileTypeTag` | approved |
| ✅ | X40 | `Components/Chat/ImageGallery` | approved |
| ✅ | X41 | `Components/Actions & inputs/Picker` | approved |
| ✅ | X42 | `Components/Chat/TopicHeader` | approved |
| ✅ | X43 | `Components/Chat/JumpToLatest` | approved |
| ✅ | X44 | `Components/Chat/BackgroundReportCard` | approved |
| ✅ | X45 | `Components/Chat/AudioPlayer` | approved |
| ✅ | X46 | `Components/Chat/VideoPlayer` | approved |
| ✅ | X47 | `Components/Chat/InteractivePage` | approved |
| ✅ | X48 | `Components/Chat/RunDetail` | approved |
| ✅ | X49 | `Components/Rich text/Prose` | approved |
| ✅ | X50 | `Components/Rich text/EditorToolbar` | approved |
| ✅ | X51 | `Components/Rich text/OutlineNav` | approved |
| ✅ | X52 | `Components/Rich text/DiffView` | approved |
| ✅ | X53 | `Components/Agents & dreams/AgentCard` | approved |
| ✅ | X54 | `Components/Agents & dreams/SetupStepCard` | approved |
| ✅ | X55 | `Components/Agents & dreams/AgentPreviewCard` | approved |
| ✅ | X56 | `Components/Agents & dreams/DreamJournalEntry` | approved |
| ✅ | X57 | `Components/Agents & dreams/PermissionRow` | approved |
| ✅ | X58 | `Components/Agents & dreams/AgentStartingCard` | approved |
| ✅ | X59 | `Components/Integrations & settings/IntegrationCard` | approved |
| ✅ | X60 | `Components/Integrations & settings/IntegrationLogo` | approved |
| ✅ | X61 | `Components/Integrations & settings/ToolList` | approved |
| ✅ | X62 | `Components/Integrations & settings/ServerRow` | approved |
| ✅ | X63 | `Components/Integrations & settings/ProviderLogo` | approved |
| ✅ | X64 | `Components/Integrations & settings/ProviderRow` | approved |
| ✅ | X65 | `Components/Integrations & settings/ProviderTile` | approved |
| ✅ | X66 | `Components/Integrations & settings/ProfileHeader` | approved |
| ✅ | X67 | `Components/Integrations & settings/RoleRow` | approved |
| ✅ | X68 | `Components/Visualisation/ActivityGrid` | approved |
| ✅ | X69 | `Components/Visualisation/NightStrip` | approved |
| ✅ | X70 | `Components/Visualisation/StackedBarChart` | approved |
| ✅ | X71 | `Components/Visualisation/BarList` | approved |
| ✅ | X72 | `Components/Visualisation/Legend` | approved |
| ✅ | X73 | `Components/Visualisation/MemoryGraph` | approved |
| ✅ | X74 | `Components/Visualisation/DocumentDetail` | approved |
| ✅ | X75 | `Components/Mobile/MobileTopBar` | approved |
| ✅ | I1, I2 | `Icons/All` (73 icons) | — |

<!-- inventory:end -->

## Final screen sweep

**2026-10-23**: every screen page of the starting design was looked at once more (Login, Onboarding, Home and its first-run and any-size frames, Chat with its file preview and the A–I flow lanes, Core, Memory and all bundles, Tasks, Skills and the skill editor, Dreams, Usage, Config, New agent, Channels, Addons, MCP, Profile, API keys, Providers, Users, Roles, States, Mobile, Scaling). Every reusable element is in the catalogue. Two gaps were found and added: the `paper-plane` icon (Telegram, in the conversation picker) and middle truncation for long names and paths (`MiddleTruncate`, used by ListItem's mono titles), from the Scaling sheet's "Long names" rule.

## Licence

MIT. The bundled fonts (Schibsted Grotesk, JetBrains Mono) keep their own OFL
licences, carried by their Fontsource packages.
