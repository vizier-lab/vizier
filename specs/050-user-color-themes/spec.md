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
3. **Given** a preset has been chosen, **When** the person switches between system, light and dark with the existing control, **Then** the preset's matching light or dark variant shows and the scheme stays selected.
4. **Given** a person has never opened Appearance, **When** they use Vizier after this feature ships, **Then** it looks exactly as it did before, and their existing light, dark or system choice is kept.

---

### User Story 2 - Make a custom color scheme (Priority: P2)

A person who wants something of their own starts from any existing scheme, gives the copy a name, changes its colors while watching a live preview, and saves it. The saved scheme appears beside the presets and is picked the same way. They can later edit, rename, duplicate or delete it.

**Why this priority**: This is the second half of the request. It builds on the scheme selection from Story 1.

**Independent Test**: Duplicate a preset, change colors in both the light and dark variants, save, select it, reload. The custom colors persist and the original preset is unchanged.

**Acceptance Scenarios**:

1. **Given** any scheme, **When** the person duplicates it, **Then** a new custom scheme starts with identical colors and needs a unique name.
2. **Given** a custom scheme is being edited, **When** a color changes, **Then** the preview updates immediately, for the light and the dark variant alike.
3. **Given** unsaved edits, **When** the person cancels or leaves the editor, **Then** the saved scheme and the active colors are unchanged, and they are warned before unsaved edits are discarded.
4. **Given** a preset, **When** the person tries to change it, **Then** they cannot: presets are read-only and the only route is to duplicate.
5. **Given** the active custom scheme, **When** it is deleted after confirmation, **Then** Vizier returns to the default scheme.
6. **Given** a color pair that falls below the legibility threshold, **When** the person is editing, **Then** they see a warning naming the affected pair, and may still save.
7. **Given** the editor is open, **When** the person changes a key color, **Then** the roles derived from it change with it.
8. **Given** a role overridden in the advanced section, **When** a key color changes, **Then** the override keeps its value until the person resets that role to its derived value.

---

### User Story 3 - Move a custom scheme between browsers (Priority: P3)

Because schemes live only on the device, a person exports a custom scheme to a file and imports it in another browser or in the desktop app, or shares the file with someone else.

**Why this priority**: Client-only storage means clearing browser data loses a custom scheme and nothing follows a person to another device. Export and import is the only remedy that needs no server, but the feature is useful without it.

**Independent Test**: Export a custom scheme, clear the site's data or open another browser, import the file. The scheme returns with identical colors.

**Acceptance Scenarios**:

1. **Given** a custom scheme, **When** it is exported and then imported elsewhere, **Then** the imported scheme's colors are identical.
2. **Given** a file that is malformed, is the wrong kind of file, or holds a value that is not a color, **When** it is imported, **Then** it is refused with a message saying what is wrong, and nothing changes.
3. **Given** an imported scheme whose name is already taken, **When** it is imported, **Then** it is added under a distinct name and overwrites nothing.

---

### Edge Cases

- **Unreadable scheme**: A custom scheme with, say, text the same color as its background. The control that returns to the default scheme stays legible whatever the active scheme is, so a person can always recover from Appearance.
- **Damaged saved data**: Saved scheme data that is corrupt or was edited by hand. Vizier falls back to the default scheme, stays usable, and does not crash. Damaged custom schemes are dropped individually where the others can still be read.
- **Device storage unavailable or full** (private window, blocked site data): The choice applies for the current session, and the person is told it could not be saved.
- **Selected scheme no longer exists**: A preset removed in a later version, or a custom scheme deleted in another tab. Vizier falls back to the default scheme.
- **Older custom scheme, newer Vizier**: A later version adds a color role that a saved scheme does not define. That role takes its value from the default scheme, and the saved scheme still loads.
- **Several tabs open**: A change in one tab reaches the others without a reload. Two tabs editing the same custom scheme: the last save wins.
- **Reduced motion**: A person who asked for less motion gets the scheme change with no animated transition, as the light and dark switch behaves today.
- **Signed-out screens**: The chosen scheme also applies to the sign-in and onboarding screens, because the choice belongs to the device and not to an account.
- **Shared browser profile**: Two people signing in on one browser profile share its schemes. Signing out does not clear them.
- **Limit reached**: At the custom scheme limit, creating or importing is refused with a message suggesting a deletion.

## Requirements *(mandatory)*

### Functional Requirements

**Choosing a scheme**

- **FR-001**: Settings MUST have an Appearance page, available to every signed-in person with no permission required, in the web and the desktop editions.
- **FR-002**: Vizier MUST ship the current look as the default scheme, plus at least four other presets.
- **FR-003**: Each scheme MUST be shown with a preview drawn from its own colors, and the active scheme MUST be clearly marked.
- **FR-004**: Selecting a scheme MUST apply it across the whole interface immediately, without a reload.
- **FR-005**: A scheme MUST define both a light and a dark variant. The existing system, light and dark choice MUST keep working independently of the scheme, and MUST decide which variant shows.
- **FR-006**: The chosen scheme MUST be restored on every later load before the first paint, with no flash of other colors.
- **FR-007**: Everything in the interface that follows the current light or dark appearance today MUST also follow the scheme, including diagrams, charts and agent-generated pages.
- **FR-008**: People who never choose a scheme MUST see no visual change, and any existing light, dark or system choice MUST be preserved.

**Custom schemes**

- **FR-009**: People MUST be able to create a custom scheme by duplicating any preset or custom scheme.
- **FR-010**: Presets MUST NOT be editable, renamable or deletable.
- **FR-011**: People MUST be able to edit, rename, duplicate and delete their custom schemes. Deleting MUST ask for confirmation.
- **FR-012**: The editor MUST present a small set of key colors (at least background, text, accent and the status colors), for the light and the dark variant separately. Changing a key color MUST update the color roles derived from it.
- **FR-012a**: The editor MUST offer an advanced section, collapsed by default, listing every color role grouped by purpose, where any role can be overridden individually.
- **FR-012b**: An overridden role MUST keep its value when a key color changes, MUST be visibly marked as overridden, and MUST be able to return to its derived value individually.
- **FR-013**: Each color MUST be settable through a visual picker and by typing a color value.
- **FR-014**: The editor MUST show a live preview of representative interface elements, and MUST NOT change the active scheme until the person saves and selects it.
- **FR-015**: A custom scheme's name MUST be 1 to 40 characters after trimming and unique among all schemes, ignoring case.
- **FR-016**: The editor MUST warn about color pairs below a 4.5:1 contrast ratio for body text and 3:1 for large text and interface elements. It MUST NOT prevent saving.
- **FR-017**: A person MUST be able to return to the default scheme in one action from Appearance, using a control that stays legible under any active scheme.
- **FR-018**: A person MUST be able to keep at least 50 custom schemes on a device.

**Export and import**

- **FR-019**: People MUST be able to export a custom scheme to a file and import such a file on any device.
- **FR-020**: Import MUST accept color values only. Anything else in a file MUST be refused or ignored, and no content from a file may be treated as anything other than a color or a display name.
- **FR-021**: A refused import MUST explain why and leave existing schemes unchanged. An imported name that clashes MUST be made distinct and never overwrite an existing scheme.

**Client-side only**

- **FR-022**: Schemes and the current selection MUST be stored only on the person's device, and MUST NOT be sent to the Vizier server.
- **FR-023**: The feature MUST require no change to the server, its API, its stored data or its configuration, and MUST work against a server of the current version.
- **FR-024**: Changes MUST reach other open tabs of the same browser without a reload.
- **FR-025**: Saved data that is damaged or unavailable MUST never prevent Vizier from loading. Vizier MUST fall back to the default scheme, as described under Edge Cases.
- **FR-026**: A custom scheme saved by an earlier version MUST keep loading after an upgrade, with roles it does not define taken from the default scheme.

### Key Entities

- **Color scheme**: A named set of colors with a light and a dark variant. Either a preset (shipped with Vizier, read-only) or custom (made by the person, editable).
- **Color role**: A purpose a color serves in the interface, such as background, text, border, accent, a status or a chart series. Each variant gives each role one color.
- **Key color**: One of the small set of colors a person edits first. Other roles are derived from the key colors unless overridden.
- **Appearance selection**: What the device remembers: which scheme is active, alongside the existing system, light or dark choice.
- **Scheme file**: The portable form of one custom scheme: its name, its colors and a format version.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A person can find Appearance and switch to a preset in under 30 seconds, with the change visible immediately.
- **SC-002**: A person can duplicate a preset, change a color and save a custom scheme in under 2 minutes.
- **SC-003**: 100% of screens show the selected scheme, with no leftover default colors in a walk through every top-level screen in both light and dark.
- **SC-004**: After a reload, no frame shows colors other than the selected scheme.
- **SC-005**: Every shipped preset meets 4.5:1 for body text and 3:1 for large text and interface elements, in both variants.
- **SC-006**: An export followed by an import reproduces every color exactly.
- **SC-007**: No request to the server contains scheme data, and a network trace of the whole flow shows no new server traffic.
- **SC-008**: People who never open Appearance see a pixel-identical interface compared with the previous release.
- **SC-009**: In a test with damaged saved data, Vizier loads with the default scheme every time.

## Assumptions

- **Colors only**: A scheme changes colors. Fonts, spacing, corner radius and motion are out of scope.
- **Light and dark stay separate**: A scheme carries both variants rather than being one or the other, because Vizier already has a system, light and dark control and that control should keep its meaning. Someone who wants a dark-only look can set the appearance to dark and leave it there.
- **Per device, not per account**: Following the client-side request, nothing syncs across browsers or devices. Clearing site data removes custom schemes, which export and import mitigate. The web edition and the desktop app store separately.
- **WebUI and desktop app only**: The website, documentation and component catalogue keep their own look. The desktop app's native window frame continues to follow light or dark only.
- **Warn, don't block**: Contrast problems warn and never stop a save. It is the person's own interface.
- **Preset names and palettes** are a design decision, made during the design pass.
- **Design first**: As a new screen and new components, this is drawn in FigCraft before any code, per the project's design process, then built in the design system and only then used in the WebUI.
- **Saved agent-generated pages** follow the scheme when they read interface colors. Colors an agent wrote explicitly into a page are left as written.
