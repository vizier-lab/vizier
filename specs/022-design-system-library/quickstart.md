# Quickstart: Design System Library

How to check that the feature works. Each step names the spec item it proves. There is no dummyplug part: this feature changes nothing agent-observable (see the plan's Testing note).

## 1. vizier is unaffected without the submodule (FR-003, SC-006)

```sh
git clone https://github.com/vizier-lab/vizier.git /tmp/vz-plain
cd /tmp/vz-plain
git submodule status          # expect: -<sha> design   (leading "-" = not initialised)
ls design                      # expect: empty
just install && cargo build    # expect: builds exactly as before
cargo package --list | grep '^design/'   # expect: no output
```

## 2. vizier with the submodule (US2, FR-002, FR-005)

```sh
git clone --recurse-submodules https://github.com/vizier-lab/vizier.git /tmp/vz-full
cd /tmp/vz-full
git submodule status          # expect:  <sha> design (heads/main)  — the pinned commit
cat .gitmodules               # expect: url = https://github.com/vizier-lab/design.git
```

Bumping the pin is a normal vizier commit:

```sh
cd design && git pull origin main && cd ..
git add design && git commit -m "chore: bump design library"
```

## 3. Library on its own: clone to catalogue in three commands (US1, FR-004, SC-001)

```sh
git clone https://github.com/vizier-lab/design.git && cd design
npm install
npm run storybook             # opens http://localhost:6006
```

Expect the sidebar to show Foundations, Icons, Brand and Components, with nothing missing. Time it: under 5 minutes, including the install.

## 4. Review a component (US1 scenarios 1–4, FR-019–FR-021)

For any entry, say **Components/Feedback/Notice (D25)**:

1. The docs page header shows `D25`, the FigCraft source (Design System › Notice), the drawn-on frames, the WebUI file it replaces, and the FigCraft PNG next to the live render.
2. Toolbar theme: switch Dark → Light → System. The render follows without a reload, and System tracks the OS setting.
3. The **States** story shows Default / Hover / Pressed / Focus / Disabled side by side (pseudo-states).
4. Tab through the Default story with the keyboard only: a 2 px focus ring at a 2 px offset; text fields focus emerald.
5. The **Accessibility** panel shows no violations.
6. To approve, change `tags: ["needs-review"]` to `["approved"]` in `Notice.stories.tsx` and commit. To request changes, open an issue on `vizier-lab/design` that links the story URL.

The sidebar tag filter `needs-review` lists what's left (SC-004).

## 5. Whole-catalogue checks (SC-002, SC-003, SC-007, FR-015)

```sh
npm run typecheck
npm run lint:tokens           # expect: no output, exit 0
npm test                      # expect: every story passes; no critical a11y violations
npm run build-storybook       # expect: storybook-static/ built
```

Then open the README's inventory checklist. Every inventory id (B/D/X, plus icons) is ticked and links to its entry.

## 6. Token sync round trip (US4, FR-023, FR-024, SC-005)

1. In FigCraft, change one colour variable (for example `text/muted`, Dark), then save.
2. Run:
   ```sh
   npm run sync -- /mnt/c/Users/ganes/OneDrive/Dokumen/Vizier.figcraft
   ```
   Expect exactly one `~ --vz-text-muted dark …` line.
3. `git diff --stat` shows only `src/tokens/*`.
4. The catalogue (still running) shows the new value on Foundations/Colour and on every component that uses it.
5. Revert the change in FigCraft, then sync again: back to no diff.

## 7. FigCraft stays one-to-one (FR-013c, SC-008)

In FigCraft, the Design System page has a component for every X entry and every added icon (the inventory row names it). A last pass over the screen pages finds no reusable drawn element that isn't in the inventory. Record that pass in the README with the date.
