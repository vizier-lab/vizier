# Implementation Plan: User Color Themes

**Branch**: `050-user-color-themes` | **Date**: 2026-11-05 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/050-user-color-themes/spec.md`

## Summary

People choose a color scheme on a new Settings › Appearance page, make their own, and export or import them as hand-editable files. Everything is stored in the browser, and the Vizier server does not change.

The approach leans on what already exists: every color in the UI is a `--vz-*` CSS variable with values only in `design/src/tokens/tokens.css`. The Vizier scheme is those stylesheet values, with nothing applied on top. Any other scheme is a validated map of those variables, set as inline properties on `<html>`, where they beat the stylesheet. A small cached copy lets the existing pre-paint script apply a scheme before first paint. The rail toggle goes away, and six new `--vz-code-*` tokens separate code colors from status colors.

## Technical Context

**Language/Version**: TypeScript with React 19 and React Router 7 (SPA) in `webui/` and `design/`. A small Rust change in the desktop shell `app/src-tauri/`. No change to `crates/`.

**Primary Dependencies**: Existing only: Zustand, zod, Tailwind v4, `@vizier/design`. No new packages (R5, R6).

**Storage**: `localStorage`, four keys (see [data-model.md](./data-model.md)). `vizier.theme` keeps its meaning, so existing light and dark choices carry over with no migration.

**Testing**: vitest for the pure logic in `webui/app/lib/themes/`, Storybook stories for new design components, `npm run typecheck`, `npm run lint:tokens`. End-to-end: dummyplug agent steps in [quickstart.md](./quickstart.md).

**Target Platform**: Browsers served by the WebUI; the desktop app on Windows (WebView2), macOS (WKWebView) and Linux (WebKitGTK).

**Project Type**: Web frontend inside an existing multi-repo workspace, plus one desktop shell command.

**Performance Goals**: A scheme switch paints in one frame. The pre-paint script adds no network request and reads at most one small key. People on the default scheme pay nothing at startup: no presets are loaded (R3).

**Constraints**: No server, API, stored data or configuration change (FR-023). Pixel-identical for people who choose nothing, apart from the removed toggle and Vizier's code colors (SC-008). Only hex colors reach the DOM (FR-020).

**Scale/Scope**: 11 presets, up to 50 custom schemes (about 130 KB at the largest), 50 color roles, one new settings route and one editor route.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Rule | Status | How |
|---|---|---|
| I. Lean by default | Pass | No new dependencies. Color math is about 40 lines. Validation reuses zod, inputs reuse the native color picker. |
| II. Trait-based extensibility | Pass | No Rust variants added. Presets are data; nothing branches on a scheme id except the default having no overrides. |
| III. Self-contained | Pass | Presets are bundled in the WebUI, embedded as today. No network at runtime. |
| IV. Portability | Pass | The desktop export goes through the transport seam. Native color input is checked on all three desktop webviews (R6). |
| V. Unified errors and logging | Pass | The Rust command returns `Result<(), String>` like its neighbours and uses no `println!`. |
| Matching submodule branches | **Action needed** | `just checkout 050-user-color-themes` is the first implementation step. Today `design` and `webui` are on `develop`. On Windows, `just` recipes other than the app ones are unsupported, so run it from WSL. |
| Commit order design → webui → vizier | Pass | Follows the phases below. |
| Clippy, tests, typecheck | Pass | `just app-test` for the shell, `npm run typecheck` and `npm test` in both frontends. |
| Dummyplug e2e | Pass | The HTTP/WebUI surface changes, so [quickstart.md](./quickstart.md) scripts it. |
| Design first (CLAUDE.md) | **Blocked** | This is a new screen and new components, so it must be drawn in FigCraft before any code. The FigCraft MCP did not connect in this session. The chat mockup agreed the layout but is not the drawing. |

**Post-design re-check**: Same result. One Rust command was added after research (R7); it lives in the desktop shell, not the server, so FR-023 holds. No violations to justify.

## Project Structure

### Documentation (this feature)

```text
specs/050-user-color-themes/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── theme-file.md       # the exported file, a public format
│   └── local-storage.md    # keys, pre-paint script, DOM application
├── checklists/requirements.md
└── tasks.md                # next: /speckit-tasks
```

### Source Code

```text
design/                                 # submodule, committed first
├── src/tokens/tokens.css               # add --vz-code-*; Vizier's monochrome values
├── src/tokens/tokens.ts                # list code and selection roles
├── src/styles.css                      # --color-code-*
├── src/components/CodeBlock/           # tones use code tokens, not status tokens
├── src/components/Shell/Rail.tsx       # remove the Theme button and onToggleTheme
├── src/components/Avatar/Avatar.tsx    # generated faces read the scheme's colors
├── src/lib/useSchemeColors.ts          # new: one shared observer of <html> for Avatar
├── src/components/SettingsNav/         # add the "appearance" page
├── src/components/ThemeTile/           # new: one scheme, drawn from its own colors
├── src/components/ColorField/          # new: picker plus hex input
└── src/components/ThemeScope/          # new: renders children in another scheme's colors

webui/                                  # submodule, committed second
├── app/lib/themes/
│   ├── roles.ts                        # the 50 roles and the 7 key colors
│   ├── color.ts                        # hex, OKLCH, contrast
│   ├── derive.ts                       # key colors → roles
│   ├── vizier.ts                       # parses tokens.css into Vizier's two variants
│   ├── presets.ts                      # palette data (lint-tokens-exempt)
│   ├── apply.ts                        # scheme → <html> properties and paint cache
│   ├── file.ts                         # the theme file: parse, serialize, validate
│   └── *.test.ts
├── app/stores/appearance.ts            # selection, custom schemes, tab sync
├── app/stores/session.ts               # theme code moves out; toggleTheme removed
├── app/root.tsx                        # pre-paint script applies the paint cache
├── app/lib/useDark.ts                  # adds useAppearanceKey
├── app/screens/shared/Mermaid.tsx      # redraws on any scheme change
├── app/screens/shell/ShellRail.tsx     # stops passing onToggleTheme
├── app/api/transport/                  # saveText on http and tauri
├── app/routes/settings-appearance.tsx
├── app/routes/settings-appearance-edit.tsx
└── app/screens/appearance/             # page sections and editor

app/src-tauri/src/shell.rs              # save_text command, committed with the pins
docs/src/configuration/themes.md        # how to use the page and the file format
```

**Structure Decision**: Palette data and logic live in `webui/app/lib/themes/`, because only the WebUI uses them and the website must not carry them. `design/` gets the tokens and presentational components only, so it stays free of app state, and `tokens.css` remains the one home of Vizier's values.

### Delivery order

1. **Gate**: submodule branches; FigCraft drawing of Appearance, the editor and the preset tiles in both themes.
2. **Tokens** (`design`): code tokens and monochrome values, `CodeBlock`, stories. This alone delivers FR-008a and FR-008c.
3. **Story 1** (`design`, `webui`): components, presets, apply, pre-paint, Appearance page, rail toggle removed.
4. **Story 2** (`webui`): derive, editor, custom schemes.
5. **Story 3** (`webui`, `app/src-tauri`): export, import, `saveText`.
6. **Close**: docs, pin bumps, quickstart.

## Complexity Tracking

No constitution violations.

## Open risks

- **FigCraft is unavailable**, which blocks step 1. Reconnect it before `/speckit-implement`.
- **Open agent pages** keep their colors until they next mount (R8). This matches how they treat the light/dark switch today, but a person will notice it on a scheme change.
- **Preset palettes in the chat mockup were from memory.** `presets.ts` must take its values from the sources listed in R9, and the contrast test decides any adjustment.
