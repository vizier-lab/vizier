# Data Model: Design System Library

**Feature**: [spec.md](spec.md) · **Research**: [research.md](research.md) · **Inventory**: [inventory.md](inventory.md)

The library has no runtime data. These are the entities its files describe and the rules that keep them consistent with FigCraft. Each one lives in a file, named below.

## DesignToken

One named design value, written by hand from the FigCraft variable of the same name (research R4). The library has no link to FigCraft (FR-023).

| Field | Type | Rule |
|-------|------|------|
| `name` | string | The FigCraft variable name, e.g. `bg/surface`, `radius/md` |
| `cssVar` | string | From the variable's `codeSyntax.WEB`, e.g. `--vz-surface`. Unique. Must start with `--vz-` |
| `collection` | `Color` \| `Size` \| `Tone` | |
| `values` | `{ dark, light }` for `Color`; `{ default }` for `Size` | Every mode of the collection has a value |
| `description` | string | Copied from FigCraft; shown on the Foundations page |

- **Lives in**: `src/tokens/tokens.css`, the only place values are written. `src/tokens/tokens.ts` lists names and descriptions only; the Foundations pages read values from CSS at runtime.
- **Validation**: `doc/viewport` (hidden in FigCraft) is not a token. Every name in `tokens.ts` must resolve to a defined CSS variable in both themes; the Foundations Colour story fails `npm test` if one is empty.
- **Count (2026-10-23)**: 37 Color, 18 Size, 1 Tone.

## Tone

| Field | Type | Rule |
|-------|------|------|
| `mode` | `muted` \| `default` \| `faint` \| `inverse` \| `failed` \| `live` \| `retrying` | The 7 FigCraft `Tone` modes |
| `aliases` | DesignToken | The Color token the mode resolves to, e.g. `failed` → `--vz-failed` |

- **Lives in**: `src/tokens/tokens.css` as `.tone-<mode> { color: var(--vz-…) }`.
- **Validation**: the alias must resolve to a Color token in the same file.

## TextStyle

| Field | Type | Rule |
|-------|------|------|
| `name` | string | `Type/<role>`, e.g. `Type/ui-medium` |
| `family` | `Schibsted Grotesk` \| `JetBrains Mono` | |
| `size`, `weight`, `lineHeight`, `letterSpacing` | number | As in FigCraft; `auto` line height becomes `normal` |

- **Lives in**: `src/tokens/type.css` as `@utility type-<role>`.
- **Count**: 24, plus 2 Elevation effect styles (`shadow-popover`, `shadow-dialog`).

## Icon

| Field | Type | Rule |
|-------|------|------|
| `name` | kebab-case string | The FigCraft name without `Icon/`; unique |
| `lucide` | Lucide icon name | The one Lucide icon it renders; unique per entry |

- **Lives in**: `src/icons/map.ts` (name → Lucide component). `IconName` is the union of its keys.
- **Validation**: TypeScript rejects an unknown name. A story test renders every entry, so a Lucide rename fails `npm test`.

## InventoryEntry (component)

The scope unit. Each row of [inventory.md](inventory.md) is one of these.

| Field | Type | Rule |
|-------|------|------|
| `id` | `B#` \| `D#` \| `X#` | Unique, stable; never reused for something else |
| `name` | PascalCase | Component and directory name `src/components/<Name>/` (brand: `src/brand/<Name>/`) |
| `tier` | `T1`–`T4` | Build order only. A component may depend only on its own tier or lower |
| `source` | `brand` \| `design-system` \| `extracted` | Where the first build reads it from: the Brand page, the Design System page, or its screen occurrences |
| `drawnOn` | list of screen frames | Every frame it appears on. Used by FR-013b generalisation and SC-008 |
| `variants` | axes → values | From FigCraft axes plus the screen extensions |
| `mergedInto` | inventory id \| null | Set when the entry is covered by a merged component (FR-013d) |
| `replaces` | WebUI path \| null | |
| `review` | ReviewState | See lifecycle |

### Lifecycle

```text
planned ──(built + stories)──► needs-review
                                                                                              │
                                     approved ◄──(maintainer signs off)──┬──────────────────┤
                                                                         └─ changes-requested ┘
                                                                            (issue on vizier-lab/design,
                                                                             fixed, back to needs-review)
```

- Every entry starts at `planned` (an approved inventory row). There is no FigCraft step (FR-013c).
- **Approved** is recorded as the story tag `approved`. **changes-requested** is an open issue that links the story, and the story keeps `needs-review`.
- A component may not be `approved` while a component it's built from is not (T1 before T2, …).

## Story

One catalogue entry.

| Field | Type | Rule |
|-------|------|------|
| `title` | string | `Foundations/…`, `Icons/…`, `Brand/…`, `Components/<Group>/<Name>` |
| `parameters.vizier` | StoryMeta | See [contracts/story-metadata.md](contracts/story-metadata.md) |
| `tags` | string[] | Exactly one of `needs-review` \| `approved`, plus `autodocs` |
| stories | per variant | Every variant axis value appears in at least one story. A `States` story shows each interaction state side by side (pseudo-states) |
