---
description: "Task list for user color themes"
---

# Tasks: User Color Themes

**Input**: Design documents from `specs/050-user-color-themes/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: Included where the plan or spec relies on them: unit tests for the pure logic in `webui/app/lib/themes/`, because SC-005, SC-006 and SC-009 are defined by what they check. UI is checked through Storybook and [quickstart.md](./quickstart.md).

**Organization**: Tasks are grouped by user story, so each can be built and tested on its own.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story the task belongs to

## Path Conventions

`design/` and `webui/` are git submodules. Commit in the order design → webui → vizier using `just commit`. Every commit message follows Conventional Commits.

---

## Phase 1: Setup (Gates)

**Purpose**: The gate from the plan's Constitution Check. No code before it passes. The design is [mockup.html](./mockup.html): open it before building any UI.

- [ ] T001 Run `just checkout 050-user-color-themes` from WSL to create the branch in `design`, `webui`, `landing` and `dummyplug`; confirm with `just status`

**Checkpoint**: Branches exist.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Code tokens and the theme library every story reads.

**⚠️ CRITICAL**: No user story work begins until this phase is complete.

### Tokens (`design`)

- [ ] T002 Add `--vz-code-keyword`, `-string`, `-number`, `-comment`, `-function` and `-punctuation` to all three theme blocks of `design/src/tokens/tokens.css` with the values in research R10
- [ ] T003 Map the six tokens as `--color-code-*` in `design/src/styles.css`
- [ ] T004 [P] Add the six code tokens in a new `code` group, plus `state/selection` and `state/selection-text`, to `colorTokens` in `design/src/tokens/tokens.ts`
- [ ] T005 Point the `TONE` map at the new tokens in `design/src/components/CodeBlock/CodeBlock.tsx`, leaving unhighlighted code as `text-soft`, and check the story in both themes
- [ ] T006 Run `npm run typecheck`, `npm run lint:tokens` and `npm test` in `design/`, then commit `feat(tokens): add code colors with monochrome Vizier values`

### Theme library (`webui`)

- [ ] T007 Create `webui/app/lib/themes/roles.ts` with the 50 roles (design name, `--vz-*` variable, Tailwind name), the 7 key colors, and the `Variant` and `Scheme` types, per data-model.md
- [ ] T008 [P] Create `webui/app/lib/themes/color.ts`: hex validation and `#rgb` expansion, hex to OKLCH and back, mix, WCAG contrast ratio, and ΔEok. No new dependency
- [ ] T009 [P] Write `webui/app/lib/themes/color.test.ts`, including the measured values in research R8b and R10
- [ ] T010 Create `webui/app/lib/themes/vizier.ts`, parsing `@vizier/design/tokens.css?raw` into Vizier's two variants with a pure function (research R4)
- [ ] T011 Write `webui/app/lib/themes/vizier.test.ts`, which fails if any color variable in `tokens.css` is not a role in `roles.ts`
- [ ] T012 Create `webui/app/lib/themes/contrast.ts` with the pairs in data-model.md and the accent-to-live ΔEok check (minimum 0.1), returning named problems. Add `bg/ground` on `state/failed` at 4.5, since the destructive button draws its text that way

**Checkpoint**: `npm test` in `webui/` passes. People see only the new code block colors.

---

## Phase 3: User Story 1 - Pick a preset color scheme (Priority: P1) 🎯 MVP

**Goal**: Settings › Appearance lists all 11 presets, selecting one changes everything at once, and it survives reloads with no flash. The rail toggle is gone.

**Independent Test**: Quickstart sections 1 to 4: choose a preset, walk the screens, reload, and switch the OS theme, with no server involved.

### Design components (`design`)

- [ ] T013 [US1] Remove the Theme `RailButton` and the `onToggleTheme` prop from `design/src/components/Shell/Rail.tsx`, and update `design/src/components/Shell/Shell.stories.tsx`
- [ ] T014 [P] [US1] Add the `appearance` page, labelled Appearance, under Account between Profile and API keys in `design/src/components/SettingsNav/SettingsNav.tsx`
- [ ] T015 [P] [US1] Create `design/src/components/ThemeScope/` (component, index, story): sets a given variable map as both `--vz-*` and `--color-*` on its element so real components draw in those colors (research R12), and export it from `design/src/index.ts`
- [ ] T016 [P] [US1] Create `design/src/components/ThemeTile/` (component, index, story) as in `mockup.html`: a miniature in the scheme's own colors, its name, a detail line and a selected state, with a real `button` and `aria-pressed`; export it from `design/src/index.ts`
- [ ] T017 [P] [US1] Create `design/src/lib/useSchemeColors.ts` with one shared `MutationObserver` on `<html>`, per research R8a
- [ ] T018 [US1] Use `useSchemeColors` in `design/src/components/Avatar/Avatar.tsx`: the six roles in `AVATAR_COLORS` order, falling back to `AVATAR_COLORS` when none is set inline, with stories for both cases
- [ ] T019 [US1] Run `npm run typecheck`, `npm run lint:tokens`, `npm test` and `npm run build-storybook` in `design/`, then commit `feat(themes): add theme components and remove rail toggle`

### Logic (`webui`)

- [ ] T020 [US1] Create `webui/app/lib/themes/presets.ts` with the `lint-tokens-exempt` header: Vizier and Vizier Swapped from `vizier.ts`, and Oxocarbon, Catppuccin, Gruvbox, Rosé Pine, Tokyo Night, Everforest, Solarized, Nord and Dracula as keys plus overrides, **taken from the sources in research R9, not the mockup**. Nord and Dracula are dark only. Vizier Swapped overrides light `bg/surface` to `#ffffff` (research R13)
- [ ] T021 [US1] Create the minimum of `webui/app/lib/themes/derive.ts` that presets need: key colors to roles per data-model.md's Derivation table, using `color.ts` (keys-only editing arrives in US2)
- [ ] T022 [US1] Write `webui/app/lib/themes/presets.test.ts`: every preset passes `contrast.ts` in every variant it has; variants match FR-002; Vizier Swapped exchanges exactly `bg/shell` and `bg/panel`. Adjust only the failing colors, as little as possible, until it passes. Expect Solarized light body text and the Nord and Everforest accents to need it (research R8b)
- [ ] T023 [P] [US1] Create `webui/app/lib/themes/apply.ts`: pure `resolve(scheme, mode, deviceIsDark)` returning `{ appearance, dataTheme, vars }`, and `apply` per contracts/local-storage.md. Vizier Swapped resolves only the roles it changes
- [ ] T024 [P] [US1] Write `webui/app/lib/themes/apply.test.ts`: each row of data-model.md's "Which variant shows" table, Vizier resolving to no vars, and paint cache output
- [ ] T025 [US1] Create `webui/app/stores/appearance.ts` as a Zustand store: selection and mode, storage reads and writes per contracts/local-storage.md, one toast on a failed write, `storage` event sync, the device-setting listener and `paintWindow`. Move all of this out of `webui/app/stores/session.ts` and remove `theme`, `setTheme` and `toggleTheme` there
- [ ] T026 [US1] Extend the pre-paint script in `webui/app/root.tsx` to apply `vizier.scheme.paint` by the rules in contracts/local-storage.md, and initialise the appearance store at startup
- [ ] T027 [P] [US1] Stop passing `onToggleTheme` and using `toggleTheme` in `webui/app/screens/shell/ShellRail.tsx`
- [ ] T028 [P] [US1] Add `useAppearanceKey` to `webui/app/lib/useDark.ts`, observing `data-theme` and `style` on `<html>`, and use it in `webui/app/screens/shared/Mermaid.tsx` for both the redraw and the `configured` cache check

### Page (`webui`)

- [ ] T029 [US1] Add `route('settings/appearance', 'routes/settings-appearance.tsx')` to `webui/app/routes.ts`, and make `webui/app/layouts/SettingsLayout.tsx` mark it active
- [ ] T030 [US1] Create `webui/app/screens/appearance/PresetGrid.tsx` with `ThemeTile` for each preset, loading `presets.ts` lazily so default users don't pay for it, and the System/Light/Dark `Segment` shown only for schemes with both variants
- [ ] T031 [US1] Create `webui/app/screens/appearance/SchemePreview.tsx`: real design components, including `CodeBlock`, `Status` and `Button`, inside `ThemeScope`
- [ ] T032 [US1] Create `webui/app/routes/settings-appearance.tsx` with `SettingsFrame`, the grid, the preview, and a Use default button inside a `ThemeScope` fixed to Vizier (FR-017). Unlike the mockup, whose fixed sidebars clip the System/Light/Dark control at 382px, the page must have no clipping or sideways scroll at phone width
- [ ] T033 [US1] Run `npm run typecheck`, `npm run lint:tokens` and `npm test` in `webui/`, walk quickstart sections 1 to 4 against a dummyplug agent, then commit `feat(themes): add appearance settings with presets`

**Checkpoint**: Story 1 works alone and is shippable as the MVP.

---

## Phase 4: User Story 2 - Make a custom color scheme (Priority: P2)

**Goal**: Duplicate any scheme, edit it with a live preview, and save, rename, or delete it.

**Independent Test**: Quickstart section 5: a duplicated preset with changed colors persists, and the original is unchanged.

- [ ] T034 [P] [US2] Create `design/src/components/ColorField/` (component, index, story) as in `mockup.html`: native color input beside a hex text field, with an error state and an optional overridden mark and reset. Export it from `design/src/index.ts`, then commit `feat(themes): add color field`
- [ ] T035 [US2] Complete `webui/app/lib/themes/derive.ts`: `setKey(variant, key, hex)` recomputes only dependent, non-overridden roles; `overrideRole`, `resetRole`, `addVariant` and `removeVariant`, per data-model.md
- [ ] T036 [US2] Write `webui/app/lib/themes/derive.test.ts`: an override survives a key change, reset restores the derived value, a duplicate is identical until a key changes, and the last variant cannot be removed
- [ ] T037 [US2] Add custom schemes to `webui/app/stores/appearance.ts`: read and write `vizier.schemes`, validating each entry with zod and dropping only damaged ones, name rules (FR-015), the 50 limit (FR-018), create, save, duplicate, rename and delete. Deleting the selected scheme selects Vizier; saving it re-applies and rewrites the paint cache
- [ ] T038 [US2] Write `webui/app/stores/appearance.test.ts`: damaged entries, a missing selected id, name uniqueness ignoring case, the limit and a failed write
- [ ] T039 [US2] Add `route('settings/appearance/:schemeId', 'routes/settings-appearance-edit.tsx')` to `webui/app/routes.ts`
- [ ] T040 [US2] Create `webui/app/screens/appearance/KeyColorsCard.tsx`: name field with inline errors, Light/Dark `Segment`, add and remove variant, seven `ColorField`s, and warnings from `contrast.ts` naming each pair
- [ ] T041 [P] [US2] Create `webui/app/screens/appearance/AdvancedColors.tsx`: collapsed by default, every role grouped by purpose with Code as its own group, overridden marks and per-role reset
- [ ] T042 [US2] Create `webui/app/routes/settings-appearance-edit.tsx`: the draft is local state, `SchemePreview` shows it, the active scheme does not change until saved and selected (FR-014), Save and Cancel, and a warning on leaving with unsaved edits
- [ ] T043 [US2] Add a Your themes section and a Duplicate action to `webui/app/routes/settings-appearance.tsx`, with edit, rename and delete for custom schemes using a design `Dialog` to confirm
- [ ] T044 [US2] Run the checks, walk quickstart section 5, then commit `feat(themes): add custom scheme editor`

**Checkpoint**: Stories 1 and 2 work independently.

---

## Phase 5: User Story 3 - Export a theme as a template, and import it anywhere (Priority: P3)

**Goal**: Export any theme to an editable file, and import such a file as a new custom theme.

**Independent Test**: Quickstart section 6: export a preset, edit it by hand, import it; separately, export a custom scheme, clear site data, and import it.

- [ ] T045 [US3] Create `webui/app/lib/themes/file.ts` per contracts/theme-file.md: `serialize(scheme)`, and `parse(text, existingNames)` running the checks in order with their messages, zod for validation, copying only known names, gap filling and the name clash suffix
- [ ] T046 [US3] Write `webui/app/lib/themes/file.test.ts`: export then import returns identical colors (SC-006); every refusal row; `__proto__`, CSS in a value and `url(javascript:…)` refused or ignored; a partial file derives the rest; an untouched export has no overrides; the size limit
- [ ] T047 [US3] Add a `save_text` command beside `save_file` in `app/src-tauri/src/shell.rs`, using the same dialog and `desktop::safe_filename`, and register it in **both** `generate_handler!` lists. Run `just app-test`
- [ ] T048 [US3] Add `saveText(text, filename)` to `webui/app/api/transport/common.ts`, `http.ts` (blob link), `tauri.ts` (`invoke('save_text', …)`) and `index.ts`, per contracts/local-storage.md, keeping `boundary.test.ts` passing
- [ ] T049 [US3] Add Export beside Duplicate and Import in Your themes to `webui/app/routes/settings-appearance.tsx`. Import uses a hidden `<input type="file">` as `webui/app/routes/memory.tsx` does, and refuses show as an error toast. An imported scheme is not selected automatically
- [ ] T050 [US3] Run the checks, walk quickstart section 6, then commit `feat(themes): add theme export and import`

**Checkpoint**: All three stories work independently.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T051 [P] Write `docs/src/configuration/themes.md` (using Appearance, the file format and role names, what stays in the browser, credits and licences from research R9) and add it to `docs/src/SUMMARY.md`
- [ ] T052 [P] Check for colors that escape tokens with `npm run lint:tokens` in `design/` and `webui/`, and walk every top-level screen under a high-contrast custom scheme for leftovers (SC-003)
- [ ] T053 Check the remaining quickstart sections: 7 (failures) and 8 (no scheme data on the network, and `git diff --stat develop -- crates/` empty)
- [ ] T054 Run quickstart section 9 in the desktop app on Windows, and on macOS and Linux where available, including the color picker on WebKitGTK (research R6). If native picking fails there, report it; the hex field is the fallback. Say which platforms were not run
- [ ] T055 Measure the first paint on the default scheme and confirm `presets.ts` is not in the initial bundle (plan's performance goals)
- [ ] T056 Compare Vizier against the previous release for SC-008: expect differences only in the rail and code blocks
- [ ] T057 Bump the `design` and `webui` pins in the workspace and commit with the shell and docs changes as `feat(themes): user color themes` after the submodule commits are pushed with `just push`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies.
- **Foundational (Phase 2)**: Depends on Setup.
- **User Story 1 (Phase 3)**: Depends on Foundational.
- **User Story 2 (Phase 4)**: Depends on US1, because it extends the store, the page, and `derive.ts`.
- **User Story 3 (Phase 5)**: Depends on US1. It can import without the editor, but needs US2's custom scheme storage (T037), so build it after T037.
- **Polish (Phase 6)**: Depends on the stories being shipped.

### Within Each Story

- `design` tasks first, committed before the `webui` tasks that use them.
- Logic before its tests where both are listed, then pages.
- Each story ends in a check-and-commit task.

### Parallel Opportunities

- T008 and T009 alongside T004.
- T014 to T017 in `design`, each in its own folder. T013 shares no file with them.
- T023 and T024 alongside T027 and T028 in `webui`.
- T034 and T041 alongside other US2 work once T035 lands.
- T051 and T052 at the end.

## Parallel Example: User Story 1

```text
Task: "Add the appearance page in design/src/components/SettingsNav/SettingsNav.tsx"
Task: "Create design/src/components/ThemeScope/"
Task: "Create design/src/components/ThemeTile/"
Task: "Create design/src/lib/useSchemeColors.ts"
```

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Pass the gate in Phase 1.
2. Complete Phase 2, which already delivers Vizier's new code colors.
3. Complete Phase 3.
4. **Stop and validate** with quickstart sections 1 to 4. This is shippable.

### Incremental Delivery

Each story ends in its own commit and can be demonstrated alone: US1 presets, then US2 custom themes, then US3 files.

## Notes

- `[P]` means different files with no dependencies on incomplete tasks.
- Do not stage `app/src-tauri/gen/schemas/*.json` by accident. Those changes predate this feature.
- The auto-commit hooks run `git add .`, so commit by path while unrelated changes are in the tree.
