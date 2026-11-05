# Data Model: User Color Themes

All of this lives in the browser. Nothing here is a server entity.

## Role

One color purpose. There are 50, each named by its design name and bound to one variable.

| Group | Roles |
|---|---|
| `bg` (7) | ground, sidebar, shell, panel, surface, raised, pressed |
| `line` (4) | border, border-strong, focus, border-hover |
| `dot` (2) | unlit, off |
| `text` (4) | faint, muted, soft, default |
| `fill` (3) | ink, ink-hover, ink-pressed |
| `state` (17) | live, live-text, live-bg, rank, rank-text, rank-bg, failed, failed-text, failed-bg, failed-line, failed-hover, failed-pressed, retrying, retrying-bg, retrying-line, selection, selection-text |
| `data` (7) | graph-edge, graph-edge-strong, chart-1 to chart-5 |
| `code` (6, new) | keyword, string, number, comment, function, punctuation |

Plain code stays `text/soft`. Primary button text is not its own role: `Button` draws it with `text-ground`, so the ground-on-ink pair is checked below, and likewise ground on failed for the destructive button.

## Key color

The seven colors a person edits first: `content` (`bg/panel`), `frame` (`bg/shell`), `text` (`text/default`), `accent` (`fill/ink`), `live`, `retrying`, `failed`.

## Derivation

Each role lists the keys it depends on. Changing a key recomputes the non-overridden roles that depend on it. Mixing happens in OKLCH.

| Roles | Depends on | Rule |
|---|---|---|
| panel, shell, default, ink, live, retrying, failed | their key | The key itself |
| ground, surface, raised, pressed | content, text | Content moved toward text in growing steps |
| sidebar | frame, content | Halfway between |
| border, border-strong, border-hover, unlit, off, graph-edge, graph-edge-strong | content, text | Content moved further toward text |
| soft, muted, faint, focus | text, content | Text moved toward content in growing steps |
| ink-hover, ink-pressed | accent, content | Accent moved toward content |
| rank, rank-text, rank-bg, chart-1 to chart-5 | accent, content, text | Hue rotations of accent; `bg` and `text` forms like the status families |
| live-text, failed-text | their key, content | Key adjusted until 4.5:1 on content |
| live-bg, failed-bg, retrying-bg | their key, content | Key at low weight over content |
| failed-line, retrying-line | their key, content | Key at medium weight over content |
| failed-hover, failed-pressed | failed, content | Lightness shifted |
| selection, selection-text | live, content | Live darkened for contrast; text white or black by contrast |
| code keyword, string, number | accent, live, retrying | Accent, live-text and retrying |
| code function, punctuation, comment | text, content | Default, muted and faint |

Step sizes are decided in the FigCraft pass and pinned by tests against the contrast pairs.

## Variant

```ts
type Variant = {
  keys: Record<KeyColor, Hex>
  roles: Record<Role, Hex>   // always complete in storage, so reading needs no derivation
  overridden: Role[]         // keeps its value when a key changes (FR-012b)
}
```

**Rules**: every value matches `^#[0-9a-fA-F]{6}$`. A role in `overridden` is shown as overridden and can be reset, which removes it and re-derives it.

## Scheme

```ts
type Scheme = {
  id: string                 // preset slug, or "custom-" + uuid
  name: string
  kind: 'preset' | 'custom'
  light?: Variant
  dark?: Variant             // at least one of the two
}
```

| Rule | Source |
|---|---|
| Name is 1–40 characters after trimming, unique among all schemes ignoring case | FR-015 |
| Presets cannot be edited, renamed or deleted | FR-010 |
| At most 50 custom schemes; creating or importing beyond that is refused | FR-018 |
| A custom scheme keeps at least one variant | FR-012c |

**Duplicate**: copies keys and roles; `overridden` starts empty. So a copy looks identical, and a later key change re-derives only that key's dependents.

**Add variant**: derives from the Vizier keys of that appearance, with accent and live taken from the existing variant.

## Presets

| Scheme | Light | Dark | Notes |
|---|---|---|---|
| Vizier | yes | yes | From `tokens.css`; applies no overrides |
| Vizier Swapped | yes | yes | Vizier with `bg/shell` and `bg/panel` exchanged; may override `bg/surface` in light; paints only the roles it changes (research R8a) |
| Oxocarbon, Catppuccin, Gruvbox, Rosé Pine, Tokyo Night, Everforest, Solarized | yes | yes | Keys and published code colors; the rest derived |
| Nord, Dracula | no | yes | Same |

Presets are authored as `keys` plus `overrides` and expanded when the module loads.

## Appearance selection

```ts
type Mode = 'system' | 'light' | 'dark'   // vizier.theme; absent = system
type Selection = { schemeId: string; mode: Mode }   // vizier.scheme; absent = "vizier"
```

**Which variant shows**:

```text
scheme has one variant          → that variant; data-theme forced to it
scheme has both, mode is fixed  → that variant; data-theme = mode
scheme has both, mode is system → device setting; data-theme unset
```

`mode` is one remembered preference across schemes. Selecting Nord does not change it, so a light choice returns when the person selects a scheme that has light.

## States of a stored scheme

```text
absent or unreadable → Vizier following the mode (no overrides)
selected id missing  → Vizier; selection cleared
one custom damaged   → that one dropped; others kept
role missing         → re-derived from keys; if keys are damaged, the stylesheet value shows
```

## Contrast pairs

Used by the editor's warnings (FR-016) and by the preset test (SC-005).

| Foreground | Background | Minimum |
|---|---|---|
| `text/default`, `text/muted`, `text/faint` | `bg/panel` | 4.5 |
| `text/default` | `bg/surface` | 4.5 |
| `bg/ground` | `fill/ink`, `state/failed` | 4.5 |
| `state/live-text`, `state/failed-text` | `bg/panel` | 4.5 |
| `code/keyword`, `string`, `number`, `function`, `punctuation`, `text/soft` | `bg/surface` | 4.5 |
| `code/comment` | `bg/surface` | 3 |
| `line/focus`, `state/live`, `state/failed`, `state/retrying` | `bg/panel` | 3 |

Hairline borders are excluded on purpose (research R11).

One further check is a color difference, not a contrast ratio: `fill/ink` and `state/live` must be at least 0.1 apart in ΔEok, so a sigil's live dot stays visible (research R8b).
