# Research: User Color Themes

## R1. How a scheme reaches the page

**Decision**: Set each role as an inline custom property on `<html>` with `style.setProperty`. The Vizier scheme sets none, and clears any that are set.

**Rationale**: Inline properties beat every stylesheet rule, whichever `tokens.css` block is active. Tailwind's `--color-panel: var(--vz-panel)` is declared on `:root`, the same element, so utilities follow. `setProperty` takes one validated value and cannot inject a rule. A role a scheme lacks falls through to the stylesheet, which gives FR-026's fallback for free.

**Alternatives**:

- A generated `<style>` element: follows the media query without JS, but needs string-built CSS from imported data, and the pre-paint script runs before `<Links />`, so it loses source-order ties with `tokens.css`.
- Preset CSS blocks in `tokens.css`: cannot express custom schemes, so there would be two mechanisms, and the website would carry every palette.

## R2. Light, dark and the existing `data-theme`

**Decision**: Keep `vizier.theme` and `data-theme` as they are. Only single-variant schemes force the attribute to their variant. A system-following scheme leaves it unset, and the existing media listener repaints.

**Rationale**: `isDark()`, `color-scheme`, Mermaid and agent pages all read this attribute, so they stay right, including a dark-only scheme on a light device. People who used the old toggle keep their choice with no migration, because the key has not changed, which delivers FR-008 and the "remembered" edge case.

## R3. No flash on load

**Decision**: On every change, write a paint cache to `vizier.scheme.paint`: only the resolved variables. The script already inlined in `root.tsx` reads it, checks each name and value by pattern, and applies it. No cache means today's behaviour.

**Rationale**: The script cannot import presets, and inlining 11 palettes would add to every page load. The Vizier scheme has no cache, so it costs one extra `getItem`. See [contracts/local-storage.md](./contracts/local-storage.md).

## R4. Where Vizier's own values come from

**Decision**: Import `@vizier/design/tokens.css?raw`, already an exported path, and parse the `:root` and `:root[data-theme="dark"]` blocks with a small pure function. Load it lazily with the Appearance page. A test requires every color variable in `tokens.css` to be a known role.

**Rationale**: Tiles, Swapped, Duplicate and Export need both variants of Vizier at once, and `tokens.ts` states that values are never written twice. Parsing keeps one source and works in vitest. The test stops a future token from silently escaping theming.

**Alternatives**: Flipping `data-theme` and reading computed styles, as `blocks.tsx` does: inline overrides would mask the values and it fires observers. Copying values into TS: the duplicate that the design system forbids.

## R5. Validation

**Decision**: Reuse zod, already in the WebUI for the generative catalogue. Colors must match `^#[0-9a-fA-F]{6}$`; typed `#rgb` is expanded first. Only known role names are copied out of a file, so unknown keys are ignored.

**Rationale**: Hex only is enough, since all 50 role values are opaque hex today, and it makes FR-020 trivially true: a file can carry nothing but a color or a name. React escapes the name.

## R6. Color input

**Decision**: A new design `ColorField`: native `<input type="color">` beside a text field.

**Rationale**: No dependency. Native pickers are a little different per platform, and the hex field works if one misbehaves. **To verify on WebKitGTK**, the webview most likely to have a limited picker.

## R7. Saving a file on the desktop

**Decision**: Add `transport.saveText(text, filename)`. On HTTP it is a blob link. On the desktop it is a new `save_text` Tauri command using the dialog plugin the shell already uses, beside `save_file`.

**Rationale**: `saveFile` only fetches server paths, and a theme has none. Blob links rely on webview download handling, which is not dependable on WKWebView and WebKitGTK. The shell is not the server, so FR-023 holds. Import reuses the hidden `<input type="file">` pattern from `routes/memory.tsx`.

## R8. Things drawn in code

| Surface | Today | Plan |
|---|---|---|
| Design components, Tailwind utilities | Follow variables live | No change |
| Mermaid | Redraws when `useDark` changes | New `useAppearanceKey` also observes `style` on `<html>`, so it redraws on any scheme change |
| Agent pages (`InteractivePage`) | Colors baked in when mounted | No change; picks up the scheme when next mounted |
| 3D scenes (`resolvedColour`) | Read once in `useMemo` | No change; same |
| Desktop window frame | Told light or dark | Told the variant on screen |

**Decision**: Leave agent pages and 3D scenes as they are. FR-007 asks them to follow as they do today, and they already keep old colors across a light/dark switch. Remounting them would reload the page and lose its state, which is worse than a stale color. Worth a follow-up if people notice.

`InteractivePage.tsx` has its own copy of `isDark`, which still works. Leave it.

Sigils and generative UI need no change, as they draw from tokens. Sigils use `bg/ground`, `line/border-strong`, `fill/ink` and `state/live`. Generative UI's color names map to tokens in `registry/colours.ts`. Exceptions in generative UI: an agent's explicit `#rrggbb` stays as written, and the default terrain palette stays fixed because it stands for terrain.

## R8a. Generated avatars

**Decision**: `Avatar` stays self-contained in `design/`. A small `useSchemeColors` hook, with one shared `MutationObserver` on `<html>`, reads the six roles matching `AVATAR_COLORS` by position: live, ink, soft, chart-4, chart-5, border. If none of the six is set inline, it returns the fixed `AVATAR_COLORS`. If any is, it resolves all six through computed style.

**Rationale**: `boring-avatars` needs real hex values, not CSS variables. It chooses shapes from the name and picks colors by position, so the same order keeps each person's shape. Reading inline properties gives Vizier today's exact pixels, which SC-008 requires, as today's values are light-theme ones used in both themes. Avatars appear in lists, so one observer is shared rather than one each. Passing colors as a prop would mean threading them through every design component that draws an avatar.

**Consequence for Vizier Swapped**: it must paint only the roles it changes, not all 50, or it would count as having its own colors. `resolve` therefore omits roles equal to Vizier's for this one scheme, which also keeps its cache small. A custom copy of Vizier stores all its roles, so its avatars follow its colors, a slight change from the fixed set, which is acceptable for a scheme the person now owns.

## R8b. Sigil live dot

**Decision**: Require accent and live to differ by at least 0.1 in OKLab distance (ΔEok). Presets are held to this by test, and custom schemes get a warning.

**Rationale**: The dot differs from its neighbours by color alone, and a contrast ratio is the wrong measure, as two colors can have the same lightness and still be distinct. Measured: Vizier 0.437 light and 0.306 dark, Dracula 0.381, Catppuccin Mocha 0.236, Gruvbox dark 0.173. The mockup's values for Nord (`#88c0d0`, 0.095) and Everforest (`#a7c080`, 0) fail, so those presets need another accent.

## R9. Preset sources and licences

Licences were checked against each repository on 2026-11-05. All are permissive, and `docs/src/configuration/themes.md` credits them.

| Preset | Source | Licence | Variants used |
|---|---|---|---|
| Oxocarbon | `nyoom-engineering/oxocarbon` | MIT | Light, dark |
| Catppuccin | `catppuccin/catppuccin` | MIT | Latte, Mocha |
| Gruvbox | `morhetz/gruvbox` | MIT/X11, stated in its README; GitHub detects none as there is no licence file | Light, dark |
| Rosé Pine | `rose-pine/rose-pine-theme` | MIT | Dawn, Main |
| Tokyo Night | `enkia/tokyo-night-vscode-theme` | MIT | Light, Night |
| Everforest | `sainnhe/everforest` | MIT | Light, dark |
| Solarized | `altercation/solarized` | MIT | Light, dark |
| Nord | `nordtheme/nord` | MIT | Dark only |
| Dracula | `dracula/dracula-theme` | MIT | Dark only |

Prefer `enkia/tokyo-night-vscode-theme` (MIT) over `folke/tokyonight.nvim` (Apache-2.0) to keep one licence and no NOTICE obligation. Its light variant is called Light, and is the one used; Day is `folke`'s. Take the colors from these sources, not from the mockup, whose values for these presets are from memory.

## R10. Vizier's code colors

Measured against `bg/surface`, the code block's ground. Plain code is `text/soft`, as `CodeBlock` draws it today, and keeps its value.

| Kind | Light | Ratio | Dark | Ratio |
|---|---|---|---|---|
| Function | `#111114` | 17.15 | `#ffffff` | 18.22 |
| Number | `#2e2e33` | 12.29 | `#ededef` | 15.59 |
| Plain | `#3f3f46` | 9.50 | `#d6d6dc` | 12.59 |
| Keyword | `#5b5b66` | 6.10 | `#a8a8b2` | 7.73 |
| String (emerald) | `#046c4e` | 5.86 | `#10b981` | 7.18 |
| Punctuation | `#6e6e78` | 4.59 | `#82828c` | 4.79 |
| Comment | `#85858f` | 3.32 | `#6e6e78` | 3.61 |

All meet SC-005: 4.5:1 for code, 3:1 for comments. [mockup.html](./mockup.html) uses these values.

## R11. What "interface elements" means in the contrast rule

**Decision**: The 3:1 check in FR-016 and SC-005 covers the focus ring and status marks. It does not cover hairline borders.

**Rationale**: Vizier's own `line/border-strong` is about 1.5:1 on white. Including borders would fail the default scheme, and fixing that would break SC-008. The exact pairs are in [data-model.md](./data-model.md).

## R12. Previews and the recovery control

**Decision**: A design `ThemeScope` wrapper sets a scheme's `--vz-*` **and** `--color-*` variables on its element, so real components draw inside it. The Use default control sits in a scope fixed to Vizier, so it stays legible (FR-017).

**Rationale**: Tailwind's `--color-panel: var(--vz-panel)` is computed on `:root` and inherited as a resolved value, so overriding only `--vz-panel` on a descendant has no effect. Both have to be set. `tokens.ts` already lists each token's Tailwind name.

## R13. Vizier Swapped

**Decision**: Exchange `bg/shell` and `bg/panel` exactly, by decision of the feature's owner.

**Measured**: Shell against panel is 1.13:1 in light and 1.02:1 in dark, so the dark swap is nearly invisible, as accepted in FR-002a. In light, cards (`bg/surface`) on the swapped inset are 1.03:1, so Vizier Swapped overrides light `bg/surface` to `#ffffff`, as FR-002a allows: cards then sit against the inset at 1.13:1, the same step as the frame around it. The swapped inset also takes `text/faint` to 4.47:1, so light moves it one step, to `#6e6d77`, to keep 4.5:1. `bg/sidebar` is a different token used by side panels and is not swapped.
