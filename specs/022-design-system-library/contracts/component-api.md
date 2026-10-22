# Contract: Component API

What the WebUI rewrite can rely on from every component in `@vizier/design`. Rationale is in research R11.

## Package surface

```ts
// Components, icons, brand
import { Button, Picker, ListItem, Icon, Sigil } from "@vizier/design";
```

```css
/* Once, in the app's CSS entry */
@import "@vizier/design/styles.css";   /* fonts + tokens + type + Tailwind theme mapping */
@source "../node_modules/@vizier/design/src";  /* so Tailwind sees the library's classes */
```

- The theme is CSS only: `<html data-theme="dark|light">`, or no attribute to follow the OS. No provider component.
- `tokens.css` can be imported on its own (`@vizier/design/tokens.css`) by code that needs only the variables.
- Nothing is exported that isn't in an inventory entry, or that isn't a helper an entry needs (for example `sigilCells(agentId)`).

## Rules for every component

1. **Props follow the design's variant axes** in camelCase: `variant`, `size`, `tone`, `checked`, `selected`, `collapsed`, `active`, `state`. Values are the axis values in lowercase (`variant="destructive"`, `size="sm"`).
2. **Interaction states are CSS, not props**: Hover, Pressed and Focus come from `:hover`, `:active` and `:focus-visible`. **App states are props**: `disabled`, `loading`, `selected`, `invalid` (FigCraft "Error"), `readOnly`, `open`, `streaming`.
3. **Native props pass through**: each component renders one root element, spreads that element's props onto it, merges `className` last, and accepts `ref` (React 19).
4. **Content comes in**: text as props or `children`, collections as arrays. No fetching, no app stores, no router (FR-017). Navigation-like components take an `href` or `onSelect`.
5. **Controlled and uncontrolled**: inputs follow React's convention (`value`/`onChange`, or `defaultValue`).
6. **Accessibility**: the correct native element or ARIA role; every icon-only control requires `aria-label` (it's a required prop, enforced by TypeScript). Focus is visible by default (2 px `--vz-focus` at a 2 px offset; emerald for text entry).
7. **Tokens only**: classes resolve to tokens. Arbitrary values (`bg-[#…]`, `p-[13px]`) fail `npm run lint:tokens`, except for logo brand colours.

## Merged primitives (FR-013d)

These replace several drawn elements each, so their shape is fixed here. Other components follow the rules above, with their props set by their inventory row.

### ListItem (X24)

```ts
type ListItemProps = {
  leading?: ReactNode;            // Icon, Sigil, Avatar, ProviderLogo, Status dot
  title: ReactNode;
  titleMono?: boolean;            // skill names, roles, servers, paths
  subtitle?: ReactNode;
  meta?: ReactNode;               // right-aligned mono meta: sizes, times, counts
  trailing?: ReactNode;           // badges, buttons, chevron, ✓
  selected?: boolean;
  disabled?: boolean;
  href?: string; onSelect?(): void;   // makes the row interactive (Hover/Focus states)
} & HTMLAttributes<HTMLElement>;
```

Built on it: FileRow, ActivityItem, RevisionRow, RunRow, RecalledMemoryRow, AgentRow, ProviderRow, RoleRow, Picker rows.

### Picker (X41)

```ts
type PickerProps<T> = {
  trigger: ReactElement;                       // AgentSwitcher, TopicHeader, bundle button, …
  search: { placeholder: string; total?: number; value: string; onChange(v: string): void };
  filters?: ReactNode;                         // FilterTabs (Conversation picker)
  sections: { label?: string; items: T[] }[];  // "Recent", "All · A–Z", "Today"
  renderItem(item: T, state: { active: boolean; query: string }): ReactNode;  // ListItem
  remainder?: string;                          // "42 more · type to filter"
  footer?: ReactNode;                          // Create agent / New conversation / New bundle · All bundles →
  onSelect(item: T): void;
  open?: boolean; onOpenChange?(open: boolean): void;
};
```

Keyboard: ↑ ↓ move, Enter selects, Esc closes, typing goes to search. The list virtualises rows as they scroll (the Scaling rule). Instances: Agent, Conversation, Topic, Bundle. The Skill menu (X33) uses the same list part without the popover trigger.

### SortSelect (X12)

```ts
type SortSelectProps = {
  label: string;                // "Sort", "Break down by", "Sorted by"
  value: string; options: { value: string; label: string }[];
  onChange(v: string): void;
  look?: "bordered" | "text";   // bordered: toolbars; text: inside panels
};
```

### Status (D24, widened)

```ts
type StatusProps = {
  state: "live" | "idle" | "failed" | "retrying" | "starting" | "stopped"   // the original six
       | "answered" | "running" | "no-response" | "not-run" | "interrupted"  // task runs
       | "connected" | "reconnecting" | "disabled";                         // MCP and integrations
  label?: ReactNode;            // defaults to the state's words
  detail?: ReactNode;           // "retry in 40s", "timed out listing tools"
};
```

Each state maps to one dot shape (filled, ring, hollow, dashed, spinning) and one tone, in a single table in `Status.tsx`. A new state is added to that table and to the Status story, nowhere else.

### Icon

```ts
type IconProps = { name: IconName; tone?: ToneMode; size?: number /* 17 */ } & SVGProps<SVGSVGElement>;
// Renders the mapped lucide-react icon with strokeWidth 1.7 + absoluteStrokeWidth.
// IconName is the design vocabulary ("core", "memory", "zoom-in"), not Lucide's names.
```

Decorative by default (`aria-hidden`). Pass `title` to make it labelled.

## Stability

Until the WebUI rewrite starts consuming the library, the API may change freely; the catalogue is the review surface. After that, changes to an approved component's props are recorded in the library's CHANGELOG (conventional commits, `[**breaking**]` where applicable).
