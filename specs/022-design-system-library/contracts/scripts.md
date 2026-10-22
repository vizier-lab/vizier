# Contract: Library scripts

Command-line contracts for the scripts in `vizier-lab/design`. Each one is a single Node ESM file with no network access. Each exits `0` on success, and on failure exits `1` with a one-line `error: …` message on stderr.

## `npm run sync -- <path/to/Vizier.figcraft>` → `scripts/sync-figcraft.mjs`

Regenerates the tokens from the FigCraft file (FR-023, FR-024, SC-005).

- **Input**: a `.figcraft` file (zip containing `document.json`). Windows paths under WSL (`/mnt/c/...`) work.
- **Writes** (and only these):
  - `src/tokens/tokens.css`: `:root` Light, a dark block under `prefers-color-scheme` guarded by `:root:not([data-theme="light"])`, `:root[data-theme="dark"]`; size variables; `.tone-*` classes.
  - `src/tokens/type.css`: `@utility type-*` for each `Type/*` style; `shadow-popover` and `shadow-dialog`.
  - `src/tokens/tokens.ts`: names, descriptions and values for the Foundations pages.
  - `src/tokens/source.json`: the SyncRecord ([data-model](../data-model.md#syncrecord)).
- **Stdout**: one line per changed token (`~ --vz-muted dark #A0A0AA → #9E9EA8`, `+ --vz-new`, `- --vz-old`), then the counts. No other output when nothing changed.
- **Fails when**: the file can't be read or isn't a FigCraft zip; a variable has no `codeSyntax.WEB`; two variables share a CSS name; a Color variable is missing a mode value; a Tone alias points at something that isn't a Color variable.
- **Never**: touches components, or anything outside `src/tokens/`.

## `npm run icons` → `scripts/build-icons.mjs`

- **Input**: `src/icons/svg/*.svg` (exported from FigCraft).
- **Writes**: `src/icons/index.tsx` (one component per SVG, plus the `IconName` union and a name → component map).
- **Normalises**: stroke and fill colours to `currentColor`; strips width and height; keeps the `viewBox`.
- **Fails when**: an SVG has a viewBox other than `0 0 24 24`, or a fill colour that isn't `none`.

## `npm run lint:tokens` → `scripts/lint-tokens.mjs`

FR-015 check.

- **Scans**: `src/**/*.{ts,tsx,css}` except `src/tokens/**`.
- **Fails on**: hex, `rgb()` or `hsl()` literals; Tailwind arbitrary values (`-[…]`) for colour, spacing, radius or font size; `px` literals in CSS outside `src/tokens/`.
- **Allows**: colours inside `src/components/ProviderLogo/brands.ts` and `src/components/IntegrationLogo/brands.ts` (FR-015 exception), and `0`/`1px` hairlines.
- **Output**: `file:line: <match>` per violation.

## `npm test`

Runs every story headless (Vitest + Playwright Chromium, `@storybook/addon-vitest`) with axe. Fails on a critical a11y violation, on a render error, or on a story missing `parameters.vizier.id`.

## `npm run storybook` / `npm run build-storybook`

Catalogue on `http://localhost:6006` / static copy in `storybook-static/` (gitignored, not hosted).
