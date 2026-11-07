# Contract: Theme File

The exported file and the import format. It is public: people edit it by hand and share it, so a later version MUST keep reading version 1.

## Format

UTF-8 JSON, named `<slug>.vizier-theme.json`.

```json
{
  "format": "vizier-theme",
  "version": 1,
  "name": "Catppuccin",
  "light": {
    "keys": { "content": "#eff1f5", "frame": "#e6e9ef", "text": "#4c4f69", "accent": "#8839ef", "live": "#40a02b", "retrying": "#df8e1d", "failed": "#d20f39" },
    "roles": { "bg/ground": "#eff1f5", "bg/panel": "#eff1f5", "code/keyword": "#8839ef" }
  },
  "dark": {
    "keys": { "content": "#1e1e2e", "frame": "#181825", "text": "#cdd6f4", "accent": "#cba6f7", "live": "#a6e3a1", "retrying": "#f9e2af", "failed": "#f38ba8" },
    "roles": { "bg/ground": "#1e1e2e", "bg/panel": "#1e1e2e", "code/keyword": "#cba6f7" }
  }
}
```

The example is shortened. An exported file lists all 50 roles, by the design names in [data-model.md](../data-model.md), so it works as a template.

## Export

- Any scheme can be exported, preset or custom.
- `roles` is always complete, for every variant the scheme has.
- Nothing else is written: no ids, no `overridden` list, no timestamps.

## Import

An import is checked in this order, and the first failure refuses the whole file and changes nothing.

| Check | Refused with |
|---|---|
| File is at most 256 KB | That file is too large to be a theme. |
| Parses as JSON | That file isn't valid JSON. |
| `format` is `vizier-theme` | That file isn't a Vizier theme. |
| `version` is 1 | That theme needs a newer version of Vizier. |
| `name` is a string of 1–40 characters after trimming | Enter a theme name of 1 to 40 characters. |
| At least one of `light`, `dark` | That theme has no light or dark colors. |
| Every value under `keys` and `roles` is `#rrggbb` | `dark.roles.bg/panel` isn't a color. Use a value like #1e1e2e. |
| Custom scheme limit not reached | You've reached 50 themes. Delete one to import this. |

**Accepted and ignored**: unknown top-level keys, unknown role names and unknown key names.

**Filling gaps**, per variant:

1. A missing key takes the role of the same color if present (`text` from `text/default`), otherwise Vizier's key for that appearance.
2. Roles present in the file are kept and recorded as `overridden`, except that a file with every role, such as an untouched export, keeps them without marking them.
3. Missing roles are derived from the keys.

**Name clash**: a name already in use gets a suffix, `Catppuccin (2)`, trimmed to fit 40 characters. Nothing is overwritten.

**Result**: always a new custom scheme with a fresh id. It is not selected automatically, so a bad file can't change the page without a second action.

## Safety

- Values reach the DOM only through `style.setProperty` with a known variable name and a value that passed the pattern, so a file cannot supply CSS.
- `name` is displayed as text only.
- Only known role names are copied from the parsed file. Properties such as `__proto__` are never read into an object.
