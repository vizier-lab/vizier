# @vizier/design

Vizier's UI component library and its catalogue.

The catalogue (Storybook) is the **source of truth** for Vizier's design:
tokens, type, icons, brand pieces and components. The FigCraft "Celestial
Court" file was only the starting design; nothing in this repository reads or
writes it, and a design change is made here, in code, and reviewed in the
catalogue.

## Start

```sh
git clone https://github.com/vizier-lab/design.git && cd design
npm install
npm run storybook        # http://localhost:6006
```

Inside a vizier checkout the library lives at `design/` as a git submodule.

## Scripts

| Script | What it does |
|--------|--------------|
| `npm run storybook` | The catalogue on `http://localhost:6006` |
| `npm run build-storybook` | A static copy in `storybook-static/` (not hosted) |
| `npm run typecheck` | `tsc --noEmit` |
| `npm test` | Renders every story headless in Chromium, in both themes, with axe; fails on an accessibility violation, a render error, or an entry missing its inventory id |
| `npm run lint:tokens` | Fails on raw colours or sizes outside `src/tokens/` |

`npm test` needs Chromium once: `npx playwright install chromium`.

## Using the library

_To be written once the first tier of components lands._

## Reviewing

_To be written._

## Inventory checklist

_Generated once the catalogue is complete._

## Licence

MIT. The bundled fonts (Schibsted Grotesk, JetBrains Mono) keep their own OFL
licences, carried by their Fontsource packages.
