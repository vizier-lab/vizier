# Implementation Plan: Design System Library

**Branch**: `022-design-system-library` | **Date**: 2026-10-23 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/022-design-system-library/spec.md`

**Status**: Phase 0 and Phase 1 done. The inventory was approved on 2026-10-23: extracted components go back into FigCraft, repeated elements are merged where they fit, and nothing is cut. Ready for `/speckit-tasks`.

## Summary

Turn the FigCraft "Celestial Court" design into a React component library in a new public repository, `vizier-lab/design`, added to vizier as a submodule at `design/`, with a local Storybook for review. The scope is the approved [inventory](inventory.md): foundations (37 colour tokens in two themes, size, tone, 24 type styles, elevation, fonts), 45+ icons, 4 brand pieces, the 60 Design System components with the extensions the screens draw for them, and 75 components extracted from the screens. Tokens are generated from the `.figcraft` file by a script, and icons are exported from FigCraft as SVG. Components are styled with Tailwind v4 utilities that resolve only to tokens, and each catalogue entry shows its FigCraft source image, its inventory id and the WebUI file it will replace. vizier's own build, CI and release are untouched (verified in research R10).

## Technical Context

**Language/Version**: TypeScript 5.9 (matching `webui/`); React 19 (peer `^19`)

**Primary Dependencies**: Tailwind CSS 4.3 (`@tailwindcss/vite`); `lucide-react` 1.61 (icons, R6; `lucide-static` same version, dev-only, for the FigCraft swap); Storybook 10.6 (`@storybook/react-vite`, `addon-docs`, `addon-a11y`, `addon-themes`, `addon-vitest`) plus `storybook-addon-pseudo-states`; Vite 7; `clsx`; `@fontsource-variable/schibsted-grotesk`, `@fontsource-variable/jetbrains-mono`; dev-only `fflate` (token sync), Vitest 5 + `@vitest/browser-playwright` (a11y run)

**Storage**: N/A (static library; generated token files are committed)

**Testing**: `npm run typecheck`; `npm test` (every story rendered headless with axe, failing on critical violations, R9); `npm run build-storybook` (catalogue builds); a raw-value lint (`npm run lint:tokens`: no arbitrary Tailwind values or hex outside `src/tokens/`, FR-015). End-to-end: **N/A for dummyplug**. Nothing agent-observable changes: no agent loop, tools, sessions, channels or HTTP/WebUI surface (constitution e2e gate). quickstart.md scripts the catalogue review steps instead.

**Target Platform**: Evergreen browsers (the WebUI's targets); the catalogue runs locally on Node ≥ 20

**Project Type**: UI component library (separate git repository) + its catalogue

**Performance Goals**: Clone to open catalogue in under 5 minutes with 3 commands or fewer (SC-001). The catalogue cold-starts in under 30 s on the maintainer's machine.

**Constraints**: Offline (bundled fonts, no CDN); no raw colours or sizes outside `src/tokens/` (except brand colours inside ProviderLogo/IntegrationLogo, if the inventory review accepts that); no data fetching in components (FR-017); vizier builds unchanged without the submodule (FR-003)

**Scale/Scope**: About 140 components and 72 icons from the [inventory](inventory.md); 2 themes; each interactive component shown in every state

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle / gate | Assessment |
|------------------|------------|
| **I. Lean by default** | ✅ No build step or publish pipeline (source package, R3). Each dependency is justified in research: `fflate` replaces hand-typed tokens, the pseudo-states addon replaces test-only props, Playwright is the only way to back SC-007. Rejected: SVGR, cva, tailwind-merge, CSS-in-JS. ⚠ The library is large (about 140 components). That is the scope the maintainer asked for, and inventory proposals X12/X24/X41 fold 5 pickers, 3 sort controls and about 9 row types into shared primitives to keep it down. |
| **II. DRY via trait-based extensibility** | ✅ The same idea in UI form: shared primitives (ListItem, Picker, Card, Notice) with domain components composed on them, not copied (inventory ⚑ proposals). Variants are lookup tables per component, not branches spread across files. |
| **III. Self-contained runtime** | ✅ Nothing reaches vizier's binary in this feature. Fonts are bundled, not fetched, so the future WebUI stays offline-capable. Dev tooling (Storybook, Playwright) lives only in the `design` repository. |
| **IV. Portability** | ✅ Node/npm scripts only; no OS-specific paths. The sync script takes the `.figcraft` path as an argument and works with Windows paths under WSL. Vizier's `Cross.toml` targets are untouched. |
| **V. Unified errors & observability** | N/A (no Rust code). The scripts exit non-zero with a one-line message on bad input. |
| Clippy / tests / typecheck gates | ✅ vizier: `cargo build` unaffected (R10); the only vizier change is `.gitmodules`, the gitlink and docs. Library: typecheck, a11y test run, catalogue build, token lint. |
| Conventional commits | ✅ vizier: `chore: add design library submodule`, `doc: …`. Library repository: same convention from its first commit. |
| Manual verification of build-pipeline changes | ✅ quickstart includes a clean `git clone` (no submodules) followed by `cargo build` to prove SC-006. |
| Dummyplug e2e gate | N/A: no agent-observable change (see Testing). |
| #82 migration tracking | N/A: no migration or legacy code added. |

**Result**: PASS. No violations, so Complexity Tracking stays empty.

**Re-check after Phase 1**: PASS. The contracts add no dependency beyond research. The merged primitives (ListItem, Picker, SortSelect, Status) put the DRY principle into the API itself, and the lint keeps raw values out. The backport adds FigCraft work, not code. The only new rule is the brand-colour exception, and it is confined to two files that the lint names.

## Project Structure

### Documentation (this feature)

```text
specs/022-design-system-library/
├── spec.md              # Feature spec (clarified)
├── inventory.md         # Phase 0: component scope list (FR-013a), approved 2026-10-23
├── research.md          # Phase 0: decisions R1–R12
├── plan.md              # This file
├── data-model.md        # Phase 1: tokens, tone, type, icons, inventory entry lifecycle, stories, sync record
├── quickstart.md        # Phase 1: vizier unaffected → clone → catalogue → review → sync round trip
├── contracts/
│   ├── component-api.md  # package surface, per-component rules, merged primitives (ListItem, Picker, SortSelect, Status, Icon)
│   ├── story-metadata.md # titles, parameters.vizier, tags, required stories, design refs
│   └── scripts.md        # sync, lint:tokens, test, storybook
├── checklists/requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code

**vizier** (this repository): only the submodule pointer and docs change.

```text
.gitmodules              # [submodule "design"] path = design, url = https://github.com/vizier-lab/design.git
design/                  # gitlink → vizier-lab/design @ pinned commit
Readme.md                # "Design library" contributor note (clone with submodules, bump the pin)
docs/src/…               # same note in the contributor section of the book
```

**vizier-lab/design** (new repository):

```text
package.json             # @vizier/design, private, exports → src (R3)
tsconfig.json
README.md                # what it follows (FigCraft file + page), last sync, sync procedure, inventory checklist
LICENSE                  # MIT
.storybook/
├── main.ts              # react-vite, addons (docs, a11y, themes, vitest, pseudo-states), Tailwind plugin
├── preview.tsx          # imports styles.css; theme switch (Dark/Light/System); a11y test = error
└── blocks/Source.tsx    # docs block: inventory id, FigCraft source + PNG, replaces
scripts/
├── sync-figcraft.mjs    # .figcraft → src/tokens/{tokens.css,type.css,source.json} (R4)
└── lint-tokens.mjs      # FR-015 raw-value check
design-refs/             # <inventory id>.png exported from FigCraft at 2× (R8)
src/
├── index.ts             # public exports
├── styles.css           # fonts + tokens + type + Tailwind @theme mapping
├── tokens/              # generated: tokens.css, type.css, source.json
├── icons/               # map.ts (design name → Lucide) + Icon.tsx (tone, size) (R6)
├── brand/               # Mark, Wordmark, Sigil (+ generator), LedBoard
├── foundations/*.mdx    # Colour, Type, Size & spacing, Elevation, Tone pages
└── components/
    └── <Name>/<Name>.tsx, <Name>.stories.tsx, index.ts   # one per inventory D/X entry
```

**Structure Decision**: Two repositories. vizier gets only `.gitmodules`, a gitlink and docs. All code lives in `vizier-lab/design`, flat by component (`src/components/<Name>/`), with the catalogue grouping (Shell, Actions & inputs, Feedback, Surfaces & data, Chat, …) set by each story's `title` rather than by folders, so moving a component between groups doesn't touch imports.

## Phase plan (for /speckit-tasks)

1. **Repository & catalogue skeleton**: the `design` repository scaffold, Storybook with themes and a11y running on one placeholder story, submodule added to vizier, contributor docs. Proves US2 and SC-001/SC-006 first.
2. **Foundations**: run the sync script → tokens, type, tone; fonts; the 5 foundation pages; token lint.
3. **FigCraft backport (FR-013c)**: add each X component to the FigCraft Design System page through the MCP, and swap every `Icon/*` vector for its Lucide SVG (plus the 25 new icons), generalised from all of its drawn occurrences, with the merges applied (FR-013d). This is done tier by tier, just ahead of the code, rather than all at once, so the design and the code are reviewed together.
4. **Icons & brand**: the Lucide map and `Icon`; Mark, Wordmark, Sigil (generation rule), LED board.
5. **Components by tier**: T1 → T2 → T3 → T4, following the inventory ids, each with its stories, state grid, source block and design-ref PNG, and tagged `needs-review`.
6. **Review loop**: the maintainer reviews in the catalogue; findings become issues on `vizier-lab/design`; approval flips the tag (SC-004). The README checklist tracks inventory ids → entries (SC-002) and the final screen sweep (SC-008).

## Complexity Tracking

None.
