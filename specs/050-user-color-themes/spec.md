# Feature Specification: User Color Themes

**Feature Branch**: `050-user-color-themes`

**Created**: 2026-11-05

**Status**: Draft

**Input**: User description: "let's make user customized colorscheme/themes settings, this will allows user to pick from a few presets of color scheme or making their own preset's. this settings is saved and managed on the client side, no backend changes should happens"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Pick a preset color scheme (Priority: P1)

A person opens Settings, goes to Appearance, and sees the color schemes Vizier ships with, each shown with a preview of its real colors. They pick one and the whole interface changes to it at once. It is still in place the next time they open Vizier.

**Why this priority**: This is the smallest version that delivers the feature. Most people will only ever pick a preset, and it needs no editor.

**Independent Test**: Choose a non-default preset, navigate across several screens, reload and restart. The chosen scheme shows everywhere with no server involved.

**Acceptance Scenarios**:

1. **Given** the default scheme is active, **When** the person selects another preset, **Then** every screen shows the new colors without a reload.
2. **Given** a preset has been chosen, **When** the person reloads or reopens Vizier, **Then** the first thing painted is the chosen scheme, with no flash of the default colors.
3. **Given** a scheme with both variants is selected and set to follow the system, **When** the device switches between light and dark, **Then** the matching variant shows and the scheme stays selected.
4. **Given** a scheme with only one variant, such as Nord, **When** it is selected, **Then** that variant shows whatever the device's setting is, and no light or dark choice is offered.
5. **Given** a person has never chosen a theme and has no earlier light or dark choice, **When** they open Vizier, **Then** they get the Vizier scheme following the device's light or dark setting.
6. **Given** a person chose light or dark with the old rail control before this feature shipped, **When** they open Vizier after upgrading, **Then** they get the Vizier scheme fixed to that choice.
7. **Given** any scheme is selected, **When** a code block is on screen, **Then** its keywords, strings, numbers, comments, functions and punctuation use that scheme's code colors.
8. **Given** a scheme with its own colors is selected, **When** a person with no photo appears, **Then** their generated avatar keeps its shape and takes its colors from the scheme, and changes without a reload. **Given** the Vizier or Vizier Swapped scheme, **Then** generated avatars look as they do today. **Given** a person or agent with an uploaded picture, **Then** the picture is unchanged.
9. **Given** any preset and a running agent, **When** its sigil is on screen, **Then** the sigil follows the scheme and its live dot is clearly distinguishable from the other dots.

---

### User Story 2 - Make a custom color scheme (Priority: P2)

A person who wants something of their own starts from any existing scheme, gives the copy a name, changes its colors while watching a live preview, and saves it. The saved scheme appears beside the presets and is picked the same way. They can later edit, rename, duplicate or delete it.

**Why this priority**: This is the second half of the request. It builds on the scheme selection from Story 1.

**Independent Test**: Duplicate a preset, change colors in each variant it has, save, select it, reload. The custom colors persist and the original preset is unchanged.

**Acceptance Scenarios**:

1. **Given** any scheme, **When** the person duplicates it, **Then** a new custom scheme starts with identical colors and needs a unique name.
2. **Given** a custom scheme is being edited, **When** a color changes, **Then** the preview updates immediately, for whichever variant is being edited.
3. **Given** unsaved edits, **When** the person cancels or leaves the editor, **Then** the saved scheme and the active colors are unchanged, and they are warned before unsaved edits are discarded.
4. **Given** a preset, **When** the person tries to change it, **Then** they cannot: presets are read-only and the only route is to duplicate.
5. **Given** the active custom scheme, **When** it is deleted after confirmation, **Then** Vizier returns to the default scheme.
6. **Given** a color pair that falls below the legibility threshold, **When** the person is editing, **Then** they see a warning naming the affected pair, and may still save.
7. **Given** the editor is open, **When** the person changes a key color, **Then** the roles derived from it change with it.
8. **Given** a role overridden in the advanced section, **When** a key color changes, **Then** the override keeps its value until the person resets that role to its derived value.
9. **Given** a custom scheme with only a dark variant, **When** the person adds a light variant, **Then** it starts from derived colors and the scheme now offers both. **Given** a custom scheme with both, **When** the person removes one, **Then** it keeps the other, and the last remaining variant cannot be removed.
10. **Given** the editor is open, **When** the person changes a code color, **Then** a sample code block in the preview updates immediately, and no status color changes with it.

---

### User Story 3 - Export a theme as a template, and import it anywhere (Priority: P3)

A person exports any theme, preset or custom, to a file. They can use that file as a template, changing its colors by hand in a text editor and importing the result as a new theme. They can also import it unchanged in another browser or in the desktop app, or share it with someone else.

**Why this priority**: Client-only storage means clearing browser data loses a custom scheme and nothing follows a person to another device. Export and import is the only remedy that needs no server, and it gives people who prefer a text editor a way to make a theme, but the feature is useful without it.

**Independent Test**: Export a preset, change a color and the name in a text editor, import the file. A new custom scheme appears with that color changed and the preset is untouched. Separately, export a custom scheme, clear the site's data, import the file. The scheme returns with identical colors.

**Acceptance Scenarios**:

1. **Given** a custom scheme, **When** it is exported and then imported elsewhere, **Then** the imported scheme's colors are identical.
2. **Given** any preset, **When** the person exports it, **Then** the file lists every color role of every variant the preset has, under readable names, ready to edit.
3. **Given** an exported preset, **When** it is imported, edited or not, **Then** it becomes a new custom scheme and the preset stays read-only and unchanged.
4. **Given** a hand-edited file that leaves some roles out, **When** it is imported, **Then** those roles are derived from its key colors where it has them and otherwise taken from the Vizier scheme.
5. **Given** a file that is malformed, is the wrong kind of file, or holds a value that is not a color, **When** it is imported, **Then** it is refused with a message saying what is wrong, and nothing changes.
6. **Given** an imported scheme whose name is already taken, **When** it is imported, **Then** it is added under a distinct name and overwrites nothing.

---

### Edge Cases

- **Unreadable scheme**: A custom scheme with, say, text the same color as its background. The control that returns to the default scheme stays legible whatever the active scheme is, so a person can always recover from Appearance.
- **Damaged saved data**: Saved scheme data that is corrupt or was edited by hand. Vizier falls back to the default scheme, stays usable, and does not crash. Damaged custom schemes are dropped individually where the others can still be read.
- **Device storage unavailable or full** (private window, blocked site data): The choice applies for the current session, and the person is told it could not be saved.
- **Selected scheme no longer exists**: A preset removed in a later version, or a custom scheme deleted in another tab. Vizier falls back to the default scheme.
- **Older custom scheme, newer Vizier**: A later version adds a color role that a saved scheme does not define. That role takes its value from the default scheme, and the saved scheme still loads.
- **Several tabs open**: A change in one tab reaches the others without a reload. Two tabs editing the same custom scheme: the last save wins.
- **Reduced motion**: A person who asked for less motion gets the scheme change with no animated transition, as theme changes behave today.
- **Signed-out screens**: The chosen scheme also applies to the sign-in and onboarding screens, because the choice belongs to the device and not to an account.
- **Shared browser profile**: Two people signing in on one browser profile share its schemes. Signing out does not clear them.
- **Limit reached**: At the custom scheme limit, creating or importing is refused with a message suggesting a deletion.
- **Switching to a scheme without the chosen variant**: A person who fixed Vizier to light selects Nord, which is dark only. Nord shows in dark. Their light choice is remembered for schemes that offer it.
- **Dark-only scheme on a device set to light**: The scheme's dark variant shows, including in diagrams, charts, agent-generated pages and the desktop window frame, all of which follow the variant on screen and not the device.
- **Code with no recognised language**: It stays in the scheme's plain text color, as it does today.

## Requirements *(mandatory)*

### Functional Requirements

**Choosing a scheme**

- **FR-001**: Settings MUST have an Appearance page, available to every signed-in person with no permission required, in the web and the desktop editions.
- **FR-002**: Vizier MUST ship these presets. **Light and dark**: Vizier (the current look, and the default), Vizier Swapped, Oxocarbon, Catppuccin, Gruvbox, Rosé Pine, Tokyo Night, Everforest and Solarized. **Dark only**: Nord and Dracula.
- **FR-002a**: Vizier Swapped MUST be the Vizier scheme with the colors of the outer frame (rail and sidebars) and the inset content area exchanged exactly, in both variants, with no adjustment to those two colors. The dark variant's difference from Vizier is subtle by decision. Everything else, including its code colors, MUST match Vizier, except other colors that must change to keep content such as cards distinguishable from the inset.
- **FR-003**: Each scheme MUST be shown with a preview drawn from its own colors, and the active scheme MUST be clearly marked.
- **FR-004**: Selecting a scheme MUST apply it across the whole interface immediately, without a reload.
- **FR-005**: A scheme MUST define a light variant, a dark variant, or both. A scheme with one variant MUST always show it, whatever the device's setting is.
- **FR-005a**: The light/dark toggle in the shell rail MUST be removed. Appearance is the one place the look is chosen.
- **FR-005b**: For a scheme with both variants, Appearance MUST offer System, Light and Dark, with System the default. System follows the device, including when the device switches. For a scheme with one variant, this choice MUST NOT be offered.
- **FR-005c**: When no scheme has been chosen, Vizier MUST use the Vizier scheme following the device's setting.
- **FR-006**: The chosen scheme MUST be restored on every later load before the first paint, with no flash of other colors.
- **FR-007**: Everything in the interface that follows the current light or dark appearance today MUST also follow the scheme and the variant on screen, including sigils, diagrams, charts, generative UI, agent-generated pages and the desktop window frame.
- **FR-007a**: Generated user avatars MUST take their colors from the scheme on screen and MUST keep the shape they have for that person. Under the Vizier and Vizier Swapped schemes they MUST keep today's fixed colors. Uploaded pictures MUST NOT be recolored.
- **FR-008**: People who never choose a scheme MUST see no visual change apart from the removed rail toggle and the Vizier scheme's new code colors (FR-008c). A light or dark choice made with the old toggle MUST carry over as the Vizier scheme fixed to that choice.

**Code colors**

- **FR-008a**: Every variant of a scheme MUST define its own code colors for keywords, strings, numbers, comments, functions and punctuation, separate from its status colors. Changing one MUST NOT change the other.
- **FR-008b**: Everywhere code is highlighted today MUST use the code colors of the scheme on screen: chat messages, the activity trail and agent-generated pages.
- **FR-008c**: The Vizier scheme's code colors MUST be monochromatic: a visible range of neutral shades, so that each kind of code other than strings has its own shade and code never reads as only black or white plus green. Emerald (its live color) MUST be the only hue, used sparingly as a hint such as on strings. This deliberately changes how code looks today. Each other preset MUST use the code colors from its published palette, subject to the contrast rule in SC-005.
- **FR-008d**: A custom scheme's code colors MUST be derived from its key colors by default, and MUST be listed as their own group in the advanced section, where each can be overridden. The editor's preview MUST include a sample code block.

**Custom schemes**

- **FR-009**: People MUST be able to create a custom scheme by duplicating any preset or custom scheme.
- **FR-010**: Presets MUST NOT be editable, renamable or deletable.
- **FR-011**: People MUST be able to edit, rename, duplicate and delete their custom schemes. Deleting MUST ask for confirmation.
- **FR-012**: The editor MUST present a small set of key colors (at least background, text, accent and the status colors), separately for each variant the scheme has. Changing a key color MUST update the color roles derived from it.
- **FR-012c**: People MUST be able to add a missing variant to a custom scheme or remove one, and a custom scheme MUST always keep at least one variant.
- **FR-012a**: The editor MUST offer an advanced section, collapsed by default, listing every color role grouped by purpose, where any role can be overridden individually.
- **FR-012b**: An overridden role MUST keep its value when a key color changes, MUST be visibly marked as overridden, and MUST be able to return to its derived value individually.
- **FR-013**: Each color MUST be settable through a visual picker and by typing a color value.
- **FR-014**: The editor MUST show a live preview of representative interface elements, and MUST NOT change the active scheme until the person saves and selects it.
- **FR-015**: A custom scheme's name MUST be 1 to 40 characters after trimming and unique among all schemes, ignoring case.
- **FR-016**: The editor MUST warn about color pairs below a 4.5:1 contrast ratio for body text and 3:1 for large text and interface elements. It MUST also warn when the accent and live colors are too alike to tell apart, because a sigil's live dot would disappear. It MUST NOT prevent saving.
- **FR-017**: A person MUST be able to return to the default scheme in one action from Appearance, using a control that stays legible under any active scheme.
- **FR-018**: A person MUST be able to keep at least 50 custom schemes on a device.

**Export and import**

- **FR-019**: People MUST be able to export any scheme, preset or custom, to a file from Appearance, and import such a file on any device. An imported file MUST always become a new custom scheme.
- **FR-019a**: The exported file MUST work as a template: plain text that can be edited by hand, with every color role of each variant listed under a readable name. A hand-edited file MUST import without any other tool.
- **FR-020**: Import MUST accept color values only. Anything else in a file MUST be refused or ignored, and no content from a file may be treated as anything other than a color or a display name.
- **FR-021**: A refused import MUST explain why and leave existing schemes unchanged. An imported name that clashes MUST be made distinct and never overwrite an existing scheme.

**Client-side only**

- **FR-022**: Schemes and the current selection MUST be stored only on the person's device, and MUST NOT be sent to the Vizier server.
- **FR-023**: The feature MUST require no change to the server, its API, its stored data or its configuration, and MUST work against a server of the current version.
- **FR-024**: Changes MUST reach other open tabs of the same browser without a reload.
- **FR-025**: Saved data that is damaged or unavailable MUST never prevent Vizier from loading. Vizier MUST fall back to the default scheme, as described under Edge Cases.
- **FR-026**: A custom scheme saved by an earlier version MUST keep loading after an upgrade, with roles it does not define taken from the default scheme.

### Key Entities

- **Color scheme**: A named set of colors with a light variant, a dark variant, or both. Either a preset (shipped with Vizier, read-only) or custom (made by the person, editable).
- **Color role**: A purpose a color serves in the interface, such as background, text, border, accent, a status, a chart series or a kind of code. Each variant gives each role one color.
- **Key color**: One of the small set of colors a person edits first. Other roles are derived from the key colors unless overridden.
- **Appearance selection**: What the device remembers: which scheme is active and, for a scheme with both variants, whether it follows the system or is fixed to light or dark. No selection means the Vizier scheme following the system.
- **Scheme file**: The portable, hand-editable form of one scheme, also used as a template: its name, the variants it has, their colors by role and a format version.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A person can find Appearance and switch to a preset in under 30 seconds, with the change visible immediately.
- **SC-002**: A person can duplicate a preset, change a color and save a custom scheme in under 2 minutes.
- **SC-003**: 100% of screens show the selected scheme, with no leftover default colors in a walk through every top-level screen in each variant the scheme has.
- **SC-004**: After a reload, no frame shows colors other than the selected scheme.
- **SC-005**: Every shipped preset meets 4.5:1 for body text and code colors, and 3:1 for code comments, large text and interface elements, in every variant it has, and its accent and live colors are clearly different.
- **SC-006**: An export followed by an import reproduces every color exactly.
- **SC-007**: No request to the server contains scheme data, and a network trace of the whole flow shows no new server traffic.
- **SC-008**: People who never open Appearance see a pixel-identical interface compared with the previous release, apart from the removed rail toggle and Vizier's new monochromatic code colors.
- **SC-009**: In a test with damaged saved data, Vizier loads with the default scheme every time.
- **SC-010**: Changing any status color in a custom scheme leaves all code colors unchanged, and the reverse.

## Assumptions

- **Colors only**: A scheme changes colors. Fonts, spacing, corner radius and motion are out of scope.
- **The rail toggle goes away**: Light and dark are chosen per scheme on the Appearance page, not with a global control. A person who wants Vizier dark on a light device fixes it there.
- **Syntax themes are not separate**: Code colors belong to the scheme. Choosing a code theme independently of the interface theme is out of scope, and so is adding languages or finer-grained kinds of code than the six that exist today.
- **Per device, not per account**: Following the client-side request, nothing syncs across browsers or devices. Clearing site data removes custom schemes, which export and import mitigate. The web edition and the desktop app store separately.
- **WebUI and desktop app only**: The website, documentation and component catalogue keep their own look. The desktop app's native window frame continues to follow light or dark only.
- **Warn, don't block**: Contrast problems warn and never stop a save. It is the person's own interface.
- **Preset palettes** come from each theme's published palette. Where one misses the contrast rule in SC-005 (Solarized's light body text is about 4.1:1), only the failing colors are adjusted, as little as possible, so a preset can differ slightly from the original. How each palette fills Vizier's status and chart roles is decided while authoring the presets.
- **Preset licences** are believed to be permissive and are confirmed during planning, with any required credit added to the notices.
- **Design reference**: The agreed mockup in `mockup.html` is the design for this feature, by decision of the feature's owner, in place of a FigCraft drawing. New components are still built in the design system first and only then used in the WebUI.
- **Saved agent-generated pages** follow the scheme when they read interface colors. Colors an agent wrote explicitly into a page are left as written.
