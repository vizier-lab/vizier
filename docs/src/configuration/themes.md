# 2.20 Color Themes

**Settings › Appearance** chooses how the WebUI and the desktop app look on this device. Pick one of the themes Vizier ships with, or make your own. A theme changes colors only: not fonts, spacing or layout.

Themes live in the browser (or, in the desktop app, its webview). Nothing is sent to the server, so they do not follow you to another browser or device, and clearing site data removes the ones you made. Export the ones you would miss.

## Choosing a theme

Selecting a theme applies it at once, everywhere, and it is restored before the next page paints.

| Theme | Light | Dark |
|---|---|---|
| Vizier (the default) | yes | yes |
| Vizier Swapped | yes | yes |
| Oxocarbon, Catppuccin, Gruvbox, Rosé Pine, Tokyo Night, Everforest, Solarized | yes | yes |
| Nord, Dracula | no | yes |

A theme with both variants offers **System**, **Light** and **Dark**. System follows the device, also when the device switches. A theme with one variant always shows it, and your choice is remembered for the next theme that offers both.

**Use default** returns to Vizier following the device. It keeps Vizier's own colors, so it stays readable whatever the current theme has done to the page.

Generated avatars take their colors from the theme and keep their shape. Under Vizier and Vizier Swapped they keep their usual colors. Uploaded pictures never change.

Agent-generated pages and 3D scenes that are already open keep their colors until they are next opened, as they do when switching between light and dark. Colors an agent wrote into a page by hand stay as written.

## Making your own

**Duplicate** copies the selected theme into an editor with a live preview. The theme you are using stays on until you save and select the new one.

- **Key colors** are the seven you edit first: content, sidebar, text, accent, live, retrying and failed. Changing one recomputes the colors that follow it.
- **Show advanced** lists every color by purpose. A color you set here is marked and keeps its value when a key color changes, until you reset it.
- **Light and dark** are edited separately. You can add a missing variant or remove one, but a theme always keeps at least one.
- **Warnings** name any pair that is hard to read, below 4.5:1 for text and code or 3:1 for comments and marks, and warn when the accent and live colors are too alike for a sigil's live dot to show. They never stop you saving.

Names are 1 to 40 characters and unique, ignoring case. You can keep 50 of your own themes.

## Export and import

**Export** saves any theme, shipped or your own, as `<name>.vizier-theme.json`. **Import** adds such a file as a new theme under **Your themes**. It is not selected automatically, and a name already in use gets a suffix such as `Catppuccin (2)`.

The file is plain JSON meant to be edited by hand, so an export is also a template:

```json
{
  "format": "vizier-theme",
  "version": 1,
  "name": "My theme",
  "dark": {
    "keys": {
      "content": "#1e1e2e",
      "frame": "#181825",
      "text": "#cdd6f4",
      "accent": "#cba6f7",
      "live": "#a6e3a1",
      "retrying": "#f9e2af",
      "failed": "#f38ba8"
    },
    "roles": {
      "code/keyword": "#cba6f7"
    }
  }
}
```

- `light`, `dark` or both must be present.
- Colors are `#rrggbb` only. Anything else refuses the whole file with a message saying where the problem is, and nothing changes.
- Unknown names are ignored. A file carries colors and a name and nothing else, so it cannot run anything.
- Leave colors out and they are worked out for you: a missing key takes the color of its own role if the file gives it, otherwise Vizier's, and missing roles follow the keys.
- Files are limited to 256 KB.

### Role names

| Group | Roles |
|---|---|
| `bg/` | `ground`, `sidebar`, `shell`, `panel`, `surface`, `raised`, `pressed` |
| `line/` | `border`, `border-strong`, `focus`, `border-hover` |
| `dot/` | `unlit`, `off` |
| `text/` | `faint`, `muted`, `soft`, `default` |
| `fill/` | `ink`, `ink-hover`, `ink-pressed` |
| `state/` | `live`, `live-text`, `live-bg`, `rank`, `rank-text`, `rank-bg`, `failed`, `failed-text`, `failed-bg`, `failed-line`, `failed-hover`, `failed-pressed`, `retrying`, `retrying-bg`, `retrying-line`, `selection`, `selection-text` |
| `data/` | `graph-edge`, `graph-edge-strong`, `chart-1` to `chart-5` |
| `code/` | `keyword`, `string`, `number`, `comment`, `function`, `punctuation` |

Plain code uses `text/soft`. The keys are `content` (`bg/panel`), `frame` (`bg/shell`), `text` (`text/default`), `accent` (`fill/ink`), `live`, `retrying` and `failed`.

## Credits

The shipped palettes come from these projects, all under the MIT license. Where a published color misses the contrast rule, that color is adjusted as little as possible, so a theme can differ slightly from the original.

| Theme | Source |
|---|---|
| Oxocarbon | [nyoom-engineering/oxocarbon](https://github.com/nyoom-engineering/oxocarbon) |
| Catppuccin | [catppuccin/catppuccin](https://github.com/catppuccin/catppuccin) |
| Gruvbox | [morhetz/gruvbox](https://github.com/morhetz/gruvbox) (MIT/X11, stated in its README) |
| Rosé Pine | [rose-pine/rose-pine-theme](https://github.com/rose-pine/rose-pine-theme) |
| Tokyo Night | [enkia/tokyo-night-vscode-theme](https://github.com/enkia/tokyo-night-vscode-theme) |
| Everforest | [sainnhe/everforest](https://github.com/sainnhe/everforest) |
| Solarized | [altercation/solarized](https://github.com/altercation/solarized) |
| Nord | [nordtheme/nord](https://github.com/nordtheme/nord) |
| Dracula | [dracula/dracula-theme](https://github.com/dracula/dracula-theme) |
