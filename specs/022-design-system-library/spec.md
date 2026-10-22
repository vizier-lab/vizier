# Feature Specification: Design System Library

**Feature Branch**: `022-design-system-library`

**Created**: 2026-10-23

**Status**: Draft

**Input**: User description: "as part of our redesign effort we have the new design in figcraft, but i think before we can start the rewrite of webui, it'll be better to break down this design into a dedicated ui library. thus, can you setup a git sumodule project of the design system? create a new repo for the submodule named "design". also equip the ui library with storybook so i can review it"

## Context

The WebUI redesign ("Celestial Court") lives in the FigCraft file `Vizier.figcraft`. Its **Design System** page is the agreed source of truth: colour tokens in Dark and Light modes, size and spacing tokens, the `Tone` icon-colour modes, the `Type/*` text styles, two elevation styles, 45 `Icon/*` components, the brand mark and wordmark, and about 40 UI components (NavItem, Button, TextField, Notice, TrailCall, Sidebar, SettingsNav, …). Every interactive component has state variants (hover, pressed, disabled, loading, keyboard focus).

Before the WebUI is rewritten, this design is turned into a standalone UI library. The library lives in its own repository, named `design`, and is added to the vizier repository as a git submodule. It ships with an interactive component catalogue (Storybook) so the maintainer can review each component against the FigCraft file before any screen is rebuilt on it.

## Clarifications

### Session 2026-10-23

- Q: Repository owner and visibility? → A: The maintainer created `vizier-lab/design`, which is public.
- Q: Component scope for the first release? → A: Every non-screen component, including composites, plus components that exist only on screen pages and were never added to the Design System page. These are extracted and included.
- Q: Where is the catalogue reviewed? → A: Locally for now.
- Q: Should extracted components also be added to the FigCraft Design System page? → A: Yes (FR-013c).
- Q: Should repeated variants (pickers, sort controls, rows, status dots) be merged? → A: Yes, where possible (FR-013d).
- Q: Can anything (charts, memory graph, rich-text editor) be cut or deferred? → A: No. Everything in the inventory is needed for the WebUI rewrite.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Review the design system in a browsable catalogue (Priority: P1)

The maintainer opens the component catalogue and checks every token, icon and component of the design system, in both themes and in every state, side by side with the FigCraft Design System page.

**Why this priority**: the user asked for this outright ("so i can review it"). It is the gate before the WebUI rewrite: nothing gets built on a component the maintainer hasn't approved.

**Independent Test**: start the catalogue from a fresh checkout. Choose any component listed on the FigCraft Design System page, switch between Dark and Light and through each state, and compare it with the FigCraft frame.

**Acceptance Scenarios**:

1. **Given** a fresh checkout of the library, **When** the maintainer runs the documented start command, **Then** the catalogue opens in a browser and lists foundations (colour, type, size/spacing, elevation, tone), icons, brand assets and components as separate groups.
2. **Given** the catalogue is open on a component, **When** the maintainer toggles the theme, **Then** the component re-renders in the other theme using that theme's token values, without a page reload.
3. **Given** a component that has state variants in FigCraft, **When** the maintainer opens its entry, **Then** every variant (default, hover, pressed, disabled, loading and keyboard focus, where the design has them) can be seen without hand-editing code, either all at once or through controls.
4. **Given** the catalogue is open, **When** the maintainer moves through a component with the keyboard only, **Then** the focus ring appears as drawn in FigCraft (2 px focus stroke, offset 2 px), and text-entry focus stays emerald.

---

### User Story 2 - Design library as its own repository, linked as a submodule (Priority: P1)

A contributor clones vizier, initialises submodules, and finds the design library at a known path, pinned to a specific library version. The library can also be cloned and worked on by itself.

**Why this priority**: it is the structural ask, and story 1 has to live somewhere.

**Independent Test**: clone vizier with submodules, check that the library is at the expected path and pinned commit. Then clone the `design` repository on its own and run its catalogue.

**Acceptance Scenarios**:

1. **Given** the vizier repository, **When** a contributor clones it with submodules, **Then** the design library is checked out at `design/`, at the commit vizier pins.
2. **Given** someone clones vizier without submodules, **When** they build and run vizier as they do today, **Then** it builds and runs unchanged: the library is not yet a build dependency.
3. **Given** the `design` repository cloned by itself, **When** a contributor installs dependencies and starts the catalogue, **Then** it works with no file from the vizier repository.
4. **Given** a new commit in the `design` repository, **When** a maintainer updates the submodule pointer in vizier, **Then** that is an ordinary, reviewable commit in vizier.

---

### User Story 3 - Tokens and components ready for the WebUI rewrite (Priority: P2)

A developer starting the WebUI rewrite imports tokens, icons and components from the library, rather than copying hex values or redrawing icons, and gets the same result as the catalogue.

**Why this priority**: this is the long-term point of the library, but no WebUI screen is rewritten in this feature. It is enough that the library *can* be consumed.

**Independent Test**: from a scratch app outside the library, consume the library through its documented entry point, render one component in each theme, and confirm it matches the catalogue.

**Acceptance Scenarios**:

1. **Given** the library, **When** a consumer uses its colour tokens, **Then** they are exposed under the same names as the FigCraft variables' code syntax (`--vz-*`), with Dark and Light values.
2. **Given** an icon component, **When** a consumer gives it a tone (muted, default, faint, inverse, failed, live, retrying), **Then** it takes that tone's colour, as the FigCraft `Tone` modes do.
3. **Given** a component, **When** a consumer reads its catalogue entry, **Then** it names the FigCraft component (or, for extracted components, the screen frames) it implements, and the current WebUI file it will replace where one exists (mapped in the inventory, FR-013a).

---

### User Story 4 - Keep the library in step with FigCraft (Priority: P3)

When the FigCraft Design System page changes, the maintainer can tell which parts of the library are out of date and bring them back in line.

**Why this priority**: drift will happen, since the FigCraft file is living documentation. It can be handled by hand at first.

**Independent Test**: change one token value in FigCraft, then follow the documented update procedure and see the change in the catalogue.

**Acceptance Scenarios**:

1. **Given** a token changed in FigCraft, **When** the maintainer follows the documented update procedure, **Then** the library's token values and the catalogue's foundations pages show the new value, and nothing else changes.
2. **Given** the library, **When** the maintainer opens its README, **Then** it states which FigCraft file and page the library follows, and the date or revision last synced.

---

### Edge Cases

- A FigCraft component with no states (static display element): its catalogue entry shows the one rendering in both themes, with no state controls.
- Opacity tints on purpose (state/rank or state/failed at 8–16% on chips and banners): the library reproduces them as tints and doesn't swap them for the `-bg` tokens.
- `text-faint` is below 4.5:1 contrast: the catalogue flags it on the colour foundations page instead of silently "fixing" it.
- An instance's text property shows the component default, not the variant's own text (FigCraft gotcha): catalogue entries set text per story rather than relying on defaults.
- Unbounded lists (bundles, roles, agents): components meant for lists show a long-list story (search, filter, pagination or a searchable picker), following the Scaling rules, not just a three-item sample.
- Hidden or documentation-only variables (`doc/viewport`) are not exported as public tokens.
- The dev-only offline provider never appears in example data or copy; Ollama is used where an example provider is needed.
- Fonts (Schibsted Grotesk, JetBrains Mono) are unavailable offline: the catalogue still renders, with a documented fallback stack, and doesn't fail.

## Requirements *(mandatory)*

### Functional Requirements

**Repository & submodule**

- **FR-001**: The UI library MUST live in the public repository `vizier-lab/design` (https://github.com/vizier-lab/design), which already exists and is empty.
- **FR-002**: The vizier repository MUST include `design` as a git submodule at the path `design/`, pinned to a specific commit.
- **FR-003**: Building, running and publishing vizier MUST NOT require the submodule in this feature. The existing WebUI and its build stay untouched.
- **FR-004**: The library MUST be self-contained: install, build and catalogue commands work from a standalone clone of `design`.
- **FR-005**: Contributor docs in vizier MUST explain how to clone with submodules and how to update the pinned version.

**Foundations**

- **FR-006**: The library MUST provide every public colour variable in the FigCraft file (37 as of 2026-10-23; the hidden `doc/viewport` is excluded), with Dark and Light values, named after the FigCraft code syntax (`--vz-*`).
- **FR-007**: The library MUST provide the size tokens (radius xs/sm/md/lg/xl/pill and the spacing scale) and the two elevation styles (popover, dialog).
- **FR-008**: The library MUST provide every `Type/*` text style (display, title, heading, prose, body, ui, small, label, mono and code roles, with their -para/-medium/-strong variants), in the FigCraft families and sizes.
- **FR-009**: The library MUST let a consumer switch the theme between Dark and Light at runtime, and MUST default to following the operating system's preference.
- **FR-010**: The library MUST provide the `Tone` system: a single icon colour that a consumer sets to muted, default, faint, inverse, failed, live or retrying.

**Assets & components**

- **FR-011**: The library MUST provide all 45 `Icon/*` icons at their design size and stroke (17 px, 1.7 stroke), coloured through Tone.
- **FR-012**: The library MUST provide the brand mark (8×8 dot V, Idle/Live states, optional grid) and the star-dot wordmark (Idle/Live), plus the 5×5 agent sigil identicon.
- **FR-013**: The library MUST provide every non-screen component of the design. That means every component on the FigCraft Design System page, composites included (Sidebar, SettingsNav, TrailCall, …), **and** every reusable element drawn on the screen pages but never made into a Design System component (for example chips, banners, the context meter ring, the memory graph, list/picker rows, empty states, dialogs). Full screens and page layouts are out of scope.
- **FR-013a**: Before any component is built, an inventory MUST be produced that lists each component, says whether it comes from the Design System page or was extracted from screens, and for extracted ones names the screen frame(s) it appears on. The maintainer reviews this inventory, and it is the scope list for SC-002.
- **FR-013b**: A component extracted from screens MUST be generalised from every place it appears (all its variants and states across screens and both themes), not copied from one occurrence. Where its occurrences differ in a way that looks unintended, the inventory flags it for the maintainer to decide rather than picking one silently.
- **FR-013c**: Every component extracted from screens, and every icon glyph drawn on screens without an `Icon/*` component, MUST be added to the FigCraft Design System page as a component before it is built, so FigCraft and the library stay one-to-one.
- **FR-013d**: Where several drawn elements are the same idea with small differences (pickers, sort controls, list rows, status dots), the library MUST provide one component that covers all of them, unless a variant can't be expressed without a special case. Then it stays separate and the inventory says why.
- **FR-014**: Every interactive component MUST implement the state model on the FigCraft page: hover, pressed, disabled (40% opacity), loading (dot loader replaces the icon), and keyboard focus (focus ring as drawn), plus emerald text-entry focus.
- **FR-015**: Components MUST use the library's tokens and type styles only. No raw colour values or one-off sizes outside the token definitions. The one exception is third-party brand colours, which are allowed only as data inside the provider and integration logo components.
- **FR-016**: Components MUST be usable by keyboard and screen reader: focusable, with correct roles and labels and visible focus.
- **FR-017**: Components MUST have no Vizier data or API dependencies. They take everything as input, so they can be rendered on their own.

**Catalogue (Storybook)**

- **FR-018**: The library MUST include a Storybook catalogue with one entry per foundation group, icon set, brand asset and component.
- **FR-019**: The catalogue MUST have a global Dark/Light theme switch that applies to every entry.
- **FR-020**: Each component entry MUST show all its variants and states, and MUST state the FigCraft component (or source screen frames) it implements and the WebUI file it is meant to replace, where one exists.
- **FR-021**: The catalogue MUST include an accessibility check per entry that the maintainer can see while reviewing.
- **FR-022**: The catalogue MUST be reviewable locally, started with one documented command. Hosting it at a shareable URL is out of scope for now.

**Sync with FigCraft**

- **FR-023**: The library's README MUST name the FigCraft file and page it follows, record the last sync, and describe how to bring changes across.
- **FR-024**: Token values MUST be defined in a single place in the library, so that a FigCraft token change is one edit.

### Key Entities

- **Design token**: a named design value (colour, radius, spacing, elevation) with a value per theme where it varies by theme. Named after the FigCraft variable's code syntax.
- **Text style**: a named typography role (family, size, weight, line height) from `Type/*`.
- **Tone**: a named icon-colour role that maps to a colour token.
- **Icon**: one of the 45 glyphs, drawn at a fixed size and stroke and coloured by Tone.
- **Component**: a reusable UI element with variants (for example Active × Collapsed) and states, linked to its FigCraft component and to the WebUI file it will replace.
- **Story**: one catalogue entry showing a component, foundation or asset in a given variant, state and theme.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: From a fresh clone of `design`, a contributor goes from clone to an open catalogue in under 5 minutes, with three documented commands or fewer.
- **SC-002**: 100% of the tokens, text styles, icons and components in the approved inventory (FR-013a: Design System page plus elements extracted from screens) have a catalogue entry. The README checklist maps each item to its entry and its FigCraft source.
- **SC-008**: After the inventory, a pass over every screen page finds no reusable, non-screen element that is missing from it.
- **SC-003**: Every in-scope component can be seen in both themes and in every state it has in FigCraft, with no code edits.
- **SC-004**: The maintainer signs off each component in one review pass. Visual differences from FigCraft that they find are logged as issues, and none are left open on a component marked approved.
- **SC-005**: A one-token change in FigCraft reaches the catalogue by editing exactly one place in the library.
- **SC-006**: Cloning and building vizier without initialising submodules works exactly as before this feature.
- **SC-007**: The catalogue reports no critical accessibility violations on any component entry.

## Assumptions

- The FigCraft Design System page (as of 2026-10-23) is the source of truth. Where FigCraft and the current WebUI disagree, FigCraft wins.
- The library targets the same front-end platform as the current WebUI, so the rewrite can consume it directly. The exact tooling is a planning decision.
- The library is consumed from the submodule path (a local link), not from a package registry. Publishing a package is out of scope for now.
- Screens (Home, Chat, Memory, Settings pages, …) are out of scope. Only the building blocks are built. Screen layouts may appear as reference compositions in the catalogue at most.
- The WebUI rewrite itself, and any change to the current WebUI, is out of scope.
- `Vizier.figcraft`, currently untracked in the vizier repository root, is not committed to vizier. Whether it moves into the `design` repository as a versioned design source is decided at planning time.
- Fonts are Schibsted Grotesk (UI and wordmark) and JetBrains Mono (mono and code), as on the Brand page, bundled with the library rather than loaded from a third-party CDN, since vizier is self-hosted.
- The maintainer has created `vizier-lab/design`. Pushing the first commits to it and adding the submodule are done in the implementation phase.
