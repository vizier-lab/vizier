# Research: Design System Library

**Feature**: [spec.md](spec.md) · **Plan**: [plan.md](plan.md) · **Inventory**: [inventory.md](inventory.md)

Versions were checked against the npm registry on 2026-10-23.

## R1. Platform: React 19 + TypeScript, matching the WebUI

- **Decision**: The library is React (peer `react`/`react-dom` `^19`) written in TypeScript **5.9**, the same majors the WebUI uses (`webui/package.json`: React 19.2, TypeScript 5.9, Vite 7, Tailwind 4.1).
- **Rationale**: The rewrite will import these components directly. One toolchain means nothing to reconcile when it does. TypeScript 7.0 (the native compiler) came out on 2026-10-22, the day before this research; the library moves to it along with the WebUI, not before.
- **Alternatives**: Web Components (framework-neutral, but the only consumer is React, and they'd add a wrapper layer). TypeScript 7 (too new; would split the toolchain).

## R2. Styling: Tailwind v4 over CSS custom properties

- **Decision**: Tokens are CSS custom properties (`--vz-*`) in `tokens.css`. A Tailwind v4 `@theme` block maps them to utilities (`bg-surface`, `text-muted`, `rounded-md`, `p-12`, …), and components style themselves with those utilities only. The type styles become `@utility type-*` classes generated from `Type/*`.
- **Rationale**: The WebUI is already on Tailwind v4, so the rewrite keeps one styling idiom. Utilities that only resolve to tokens make FR-015 (no raw values) checkable with a grep for arbitrary values (`-[#`, `-[13px]`). Tailwind v4 reads the library's sources from a consumer through `@source`, so there's no build step.
- **Alternatives**: CSS Modules (no consumer config, but a second idiom next to the WebUI's Tailwind, and raw values are harder to forbid). vanilla-extract or other CSS-in-JS (a runtime or compile plugin for no gain here).

## R3. Distribution: a source package consumed by path

- **Decision**: `package.json` name `@vizier/design`, `private: true`, with `exports` pointing at TypeScript source (`./src/index.ts`) and the CSS entry points (`./styles.css`, `./tokens.css`). Nothing is compiled or published. A consumer depends on it as `"@vizier/design": "file:../design"`.
- **Rationale**: The spec rules out a registry (Assumptions). A source package needs no dist directory, no bundler config and no versioning ceremony. The submodule commit *is* the version.
- **Alternatives**: Build to `dist/` with tsup or Vite library mode (only worth it once someone outside the submodule consumes it). npm or GitHub Packages publish (out of scope).

## R4. Tokens come from the FigCraft file, by script

- **Decision**: `scripts/sync-figcraft.mjs <path/to/Vizier.figcraft>` opens the file (a zip; `document.json` inside) and writes three committed files: `src/tokens/tokens.css` (Dark/Light colour variables, size variables, tone mapping), `src/tokens/type.css` (the 24 `Type/*` styles and 2 `Elevation/*` styles), and `src/tokens/source.json` (file name, save time, version id from `versions/index.json`, token counts). Hidden variables (`hiddenFromPublishing`, i.e. `doc/viewport`) are skipped. Names come from each variable's `codeSyntax.WEB`. The only dependency is `fflate` (zip reading, zero dependencies).
- **Rationale**: FR-024 needs one place per token, and US4/SC-005 a one-edit sync. Generating from the file removes transcription errors across 37 colours × 2 modes, and the script *is* the documented sync procedure (FR-023). Reading the zip directly works without the FigCraft app running, which is not true of the MCP. The `.figcraft` file stays out of both repos (the maintainer removed it as a binary); the script takes its path.
- **Alternatives**: Hand-maintained `tokens.css` (meets FR-024, but every sync is manual transcription). Through the FigCraft MCP (needs the app open and an agent session).
- **Evidence**: The file was parsed this way during research: 58 variables in 3 collections, 26 styles, all with code syntax.

## R5. Themes: `data-theme` with a system default

- **Decision**: `tokens.css` defines Light on `:root`, Dark under `@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) }`, and Dark again under `:root[data-theme="dark"]`. A consumer forces a theme by setting `data-theme` on `<html>`, or leaves it unset to follow the OS (FR-009). The catalogue's toolbar switch (`@storybook/addon-themes`, `withThemeByDataAttribute`) offers Dark, Light and System.
- **Rationale**: Pure CSS: no provider component and no flash on load. This matches how FigCraft frames switch the `Color` mode.
- **Alternatives**: A `ThemeProvider` context (React state for what CSS already does). The `dark:` class strategy (Tailwind-specific, and couples consumers to a class name).

## R6. Tone and icons: Lucide

- **Decision**: Icons come from **Lucide** through `lucide-react` (1.61.0). One table, `src/icons/map.ts`, maps each design name (`core`, `memory`, `dreams`, … plus the 25 screen glyphs) to a Lucide component. `<Icon name tone size>` renders it at 17 px with `strokeWidth={1.7}` and `absoluteStrokeWidth`, so the stroke is 1.7 px at any size, as in FigCraft. `tone` sets `color` through the `.tone-*` classes (F3). Lucide draws with `currentColor`, which is what the FigCraft `Tone` variable does.
- **FigCraft side**: each `Icon/*` component's vector is swapped for the real Lucide SVG (`file.import` of the SVG, then its vector moved into the existing component), keeping the component and its id, so every instance on every screen updates. The 25 extra glyphs are added as new `Icon/*` components the same way. This happens during the backport phase.
- **Rationale**: The maintainer asked for the matching existing pack. A side-by-side render on 2026-10-23 showed that all 47 drawn icons are Lucide lookalikes: same grid, round caps and joins, and the same composition, but drawn with straight segments (puzzle, key and refresh are simplified). Using Lucide gives the drawn set's exact intent, fills the 25 gaps without designing anything, keeps future icons free, and tree-shakes per icon. It also removes the SVG export step and the generator script that the hand-drawn set needed.
- **Alternatives**: Exporting the hand-drawn vectors (keeps their approximations, and 25 more would need drawing). Feather (Lucide's ancestor; unmaintained, missing about 10 of the needed glyphs). Tabler and Phosphor (different shapes; the screens would change). `lucide-static` SVGs plus a generator (an extra script for what `lucide-react` already ships).

## R7. Fonts are bundled

- **Decision**: `@fontsource-variable/schibsted-grotesk` and `@fontsource-variable/jetbrains-mono` (5.3.0), imported from `styles.css`. The fallback stacks are `ui-sans-serif, system-ui, sans-serif` and `ui-monospace, SFMono-Regular, Menlo, monospace`.
- **Rationale**: Vizier is self-hosted and must run offline (constitution III), so no Google Fonts CDN. Fontsource files are bundled into the consumer's build, which the WebUI already does for IBM Plex. Variable fonts cover every weight the type styles use in one file per family.
- **Alternatives**: Static per-weight packages (more files). Committing the font files (a licence and update chore that Fontsource already handles).

## R8. Catalogue: Storybook 10

- **Decision**: `storybook` and `@storybook/react-vite` 10.6.3 on Vite 7, with `@storybook/addon-docs` (autodocs and MDX foundation pages), `@storybook/addon-a11y` (FR-021), `@storybook/addon-themes` (FR-019) and `storybook-addon-pseudo-states` 10.6.3 (forces `:hover`, `:active`, `:focus-visible` so every state shows in one grid without touching component code, FR-020/SC-003). Run it with `npm run storybook`. `npm run build-storybook` produces a static copy for local review, which is not hosted (FR-022).
- **Sidebar**: `Foundations/` (Colour, Type, Size & spacing, Elevation, Tone), `Icons/`, `Brand/`, then `Components/<group>/<Name>`. The groups follow the inventory sections.
- **Per-story metadata**: `parameters.vizier = { id: 'D10', source: { kind: 'design-system' | 'extracted' | 'brand', figcraft: 'Design System › Button' | ['Chat › states & flow › C1', …] }, replaces: 'webui/app/components/…' | null }`. A docs block renders it at the top of each docs page (FR-020, US3-3).
- **Review status**: Storybook tags `needs-review` (the default for every new story) and `approved`. The sidebar filters by tag, so what's left to review is one click away. Approving means changing the tag in the story file, so approvals are reviewable commits (SC-004).
- **Design reference beside the code**: each entry's docs page shows a PNG of its FigCraft source (the component set, or the screen crop for X components), exported at 2× through the MCP into `design-refs/<id>.png`. The catalogue page then has the design and the implementation together (US1).
- **Alternatives**: Ladle (lighter, but no a11y or themes addons, and fewer docs features). Histoire (Vue-first). A hand-rolled gallery (rebuilds Storybook badly).

## R9. Checking accessibility across the catalogue

- **Decision**: `@storybook/addon-vitest` with Vitest 5 and `@vitest/browser-playwright` runs every story headless in Chromium, with `parameters.a11y.test = 'error'`, as `npm test`. A story with a critical axe violation fails.
- **Rationale**: SC-007 says "no critical violations on any component entry". With about 140 components and their state grids, only an automated run can show that. The a11y panel still lets the reviewer see results story by story (FR-021). Playwright and Chromium are development dependencies of the `design` repository only. They never reach vizier's binary or build, so constitution principle III is unaffected.
- **Alternatives**: `@storybook/test-runner` (the older Jest-based runner, superseded by the Vitest addon). Manual review in the a11y panel only (can't support a "100%" claim).

## R10. The submodule in vizier

- **Decision**: `git submodule add https://github.com/vizier-lab/design.git design`, using **HTTPS**, so anyone can clone it without SSH keys (the repository is public). Pin to the library's first reviewed commit. Contributor docs (`docs/` book and Readme) get a short "Design library" note: `git clone --recurse-submodules`, or `git submodule update --init design`, and how to bump the pin.
- **Why this doesn't touch vizier's build (FR-003, SC-006)**, checked:
  - `Cargo.toml` `include` is an allow-list (`/src/**`, `/webui/build/client/**`, …), so `design/` never enters the crate.
  - `build.rs` only looks at `webui/`.
  - Every workflow's `actions/checkout@v4` runs without `submodules:`, so `design/` stays an empty directory in CI.
  - The Docker image is built from release tarballs (`ADD vizier-v…tar.gz`) and never copies the source tree.
- **Later, not now**: when the rewritten WebUI imports `@vizier/design`, `npm run build` in `webui/` will need the submodule. The release and publish workflows will then need `submodules: true`, and anyone building from source will need `--recurse-submodules`. That belongs to the rewrite's spec.
- **Alternatives**: SSH URL (matches vizier's own remote, but locks out anonymous clones). A git subtree (no separate history, which the user asked for). An npm workspace inside vizier (not a separate repository).

## R11. Component conventions

- **Decision**:
  - One directory per component: `src/components/<Name>/<Name>.tsx`, `<Name>.stories.tsx`, and an `index.ts` re-export.
  - Prop names follow the FigCraft variant axes, in camelCase: `variant`, `size`, `tone`, `checked`, `collapsed`, `active`.
  - Interaction states (Hover, Pressed, Focus) come from CSS, never props. States the app decides (Loading, Disabled, Selected, Error, Read-only, Open, Streaming, …) are props.
  - React 19 `ref` as a prop. `className` is passed through and merged. Native element props are spread, so `<Button type="submit">` works.
  - Text comes in as props or children; components never fetch anything (FR-017).
  - The focus ring is `:focus-visible` with a 2 px `--vz-focus` outline at a 2 px offset; text-entry focus uses `--vz-live` (FR-014).
  - `clsx` is the only runtime helper (smaller than the alternatives, and the merge rule is simple, because utilities only map tokens).
- **Rationale**: The prop names line up with the FigCraft panel the reviewer is comparing against. CSS-driven states can be shown with pseudo-states (R8) without test-only props leaking into the API.
- **Alternatives**: `class-variance-authority` (another dependency where a plain lookup object per variant is a few lines). `tailwind-merge` (unneeded while classes only map tokens).

## R12. Licence

- **Decision**: MIT, the same as vizier (`LICENSE`, © 2026 Ganesha Danu Enastika). Bundled fonts keep their own OFL licences, which Fontsource packages carry.
