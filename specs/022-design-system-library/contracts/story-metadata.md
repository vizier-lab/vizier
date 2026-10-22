# Contract: Catalogue entries

How each Storybook entry is written, so that review (US1), source tracing (FR-020) and the README checklist (SC-002) can be done mechanically.

## Titles

| Kind | Title | Example |
|------|-------|---------|
| Foundation | `Foundations/<Page>` | `Foundations/Colour` |
| Icons | `Icons/All` (grid, with a tone switch) | |
| Brand | `Brand/<Name>` | `Brand/Sigil` |
| Component | `Components/<Group>/<Name>` | `Components/Feedback/Notice` |

The groups match the inventory sections: Shell, Actions & inputs, Feedback, Surfaces & data, Lists, Chat, Rich text, Agents & dreams, Integrations & settings, Visualisation, Mobile.

## `parameters.vizier`

```ts
type StoryMeta = {
  id: string;                               // inventory id: "D10", "X41", "B3"
  replaces?: string | null;                 // "webui/app/components/Toast.tsx"
  merges?: string[];                        // ids folded in: ["X25", "X26"] on ListItem
};
```

Set once in the default export (`meta.parameters.vizier`). The `Source` docs block renders it at the top of the docs page. Nothing in it refers to FigCraft: the catalogue is the source of truth (FR-023), and the inventory id is the link to the planning record. A story without `vizier.id` fails `npm test` (a check in `preview.tsx`).

## Tags

- `needs-review` on every new entry. When the maintainer signs off, it is replaced by `approved` in the same file. Exactly one of the two is present.
- `autodocs` on every component entry.

## Required stories per component

1. **Default**: the most common use, with realistic content (vizier's own names: Archivist, Scout, `vector-db-review`; never the dev-only offline provider; Ollama as the example local provider).
2. **One story per variant-axis value**, or one grid story that shows the full product of the axes where that's under ~30 cells.
3. **States** (interactive components): Default, Hover, Pressed, Focus (pseudo-states), plus Disabled, Loading, Selected or whichever app states the component has, side by side.
4. **Long content / scale** where the component holds lists or text: long names (middle truncation), 1,000 items (Picker, tables), empty.
5. Each story renders in both themes through the toolbar switch. No story pins a theme, except the Foundations comparison page, which shows both.
