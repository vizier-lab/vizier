# Quickstart: User Color Themes

How to check the feature end to end. The steps assume `just run` and a WebUI on `:9999`, or `npm run dev` on `:5173`, signed in, with an agent on the offline `dummyplug` provider.

Dummyplug steps are written as *send → expect*.

## Automated checks

```bash
cd design && npm run typecheck && npm run lint:tokens && npm test
```

```bash
cd webui && npm run typecheck && npm run lint:tokens && npm test
```

```bash
just app-test
```

The WebUI tests pin, among others: every token is a role; every preset meets the contrast pairs in every variant; export then import returns identical colors; hostile and damaged files are refused; `resolve` for each scheme and mode.

## 1. Defaults and upgrade (FR-005c, FR-008, SC-008)

1. In a new browser profile, open Vizier. Expect today's look, following the OS's light or dark setting, and a Theme button in the rail that switches light and dark. Select Nord, which is dark only, and expect the button to go; select Catppuccin and expect it back.
2. In the console, `localStorage.setItem('vizier.theme', 'dark')`, reload. Expect Vizier dark on a light OS, and Dark selected on the Appearance page.
3. Inspect `<html>`. Expect no `--vz-*` inline properties.

## 2. Code colors through a real turn (FR-008a to c)

1. Send `tools` → expect the tool list.
2. Send `memory_write` → expect a sample request with a fenced `json` block.
3. On Vizier, expect that block in greys with emerald on strings only.
4. Select Dracula → expect the same block in Dracula's colors, without a reload.
5. In the block, compare to a Live status on the same screen. Change a status color in a custom scheme → expect code unchanged (SC-010).

## 3. Presets and variants (User Story 1)

1. Select each preset → expect every screen to change without a reload: Home, chat, Memory, Tasks, Usage, Settings.
2. Reload on each → expect no flash of other colors. Throttle the CPU to check.
3. Select Catppuccin, choose System, and switch the OS theme → expect the variant to follow.
4. Select Nord on a light OS → expect dark, no System/Light/Dark control, and a dark desktop window frame.
5. Fix Vizier to Light, select Nord, select Vizier again → expect Light again.
6. Select Vizier Swapped → expect the rail and sidebar in the inset's color and the reverse. In dark, expect the difference to be subtle.

## 4. Drawn surfaces (FR-007)

1. Send `memory_write`, then this JSON:

   ````json
   { "tool": "memory_write", "arguments": { "bundle": "notes", "path": "diagram", "title": "Diagram", "content": "```mermaid\ngraph TD; A-->B;\n```" } }
   ````

   Expect the document saved. Check `memory_write`'s sample for the exact arguments.
2. Open that document in Memory and switch scheme → expect the diagram to redraw in the new colors.
3. Send `ui_render` then the JSON from `specs/026-generative-ui-terminal-tools/quickstart.md` → expect the page in the current scheme. Switch scheme → expect the page's own design components to follow live.
4. Send `execute_python` that calls a saved interactive page, if any are at hand. After switching scheme, expect it to keep its colors until reopened (research R8).
5. With the dummyplug agent's chat open, send `tools` and watch its sigil while the turn runs → expect the sigil in the scheme's colors with a visible live dot, in every preset.
6. On a user with no photo, select Dracula → expect the generated avatar in Dracula's colors with the same shape, without a reload. Select Vizier, then Vizier Swapped → expect the original colors both times. Compare against `AVATAR_COLORS`.
7. Upload a photo for the user and an image for the agent → expect neither to change with the scheme.

## 5. Custom schemes (User Story 2)

1. Duplicate Gruvbox → expect a copy with identical colors and a unique name required. Try an empty and a taken name → expect an inline error.
2. Change the accent → expect the preview to change and the active scheme not to.
3. In Advanced, override Keyword, then change the accent → expect Keyword unchanged and marked. Reset it → expect the derived value.
4. Set text equal to the background → expect a warning naming the pair, and Save still available.
5. Save and select it → expect the text to vanish everywhere, and **Use default** to stay legible. Use it → expect Vizier.
6. Leave the editor with unsaved changes → expect a warning.
7. Remove the light variant → expect the scheme to show dark always. Removing the last is not possible.
8. Delete the active scheme → expect a confirmation, then Vizier.

## 6. Export and import (User Story 3)

1. Export Vizier → expect a save dialog on the desktop and a download in a browser, and a file listing 50 roles per variant.
2. Change the name and `code/keyword` in a text editor, and import → expect a new unselected custom scheme with that color, and Vizier unchanged.
3. Import the original file again twice → expect `Vizier (2)` and `Vizier (3)`.
4. Export a custom scheme, clear site data, import → expect identical colors (SC-006).
5. Import each and expect refusal with its message and no change: a `.png` renamed to `.json`; `{"format":"other"}`; a role value of `red; background:url(x)`; a role value of `url(javascript:alert(1))`.
6. Import a file with `name`, `format`, `version` and three keys only → expect the rest derived.

## 7. Failure paths (FR-025, SC-009)

1. Set `vizier.schemes` and `vizier.scheme.paint` to `{oops`, reload → expect Vizier, no crash.
2. Set `vizier.scheme` to `gone`, reload → expect Vizier.
3. Fill storage until a write fails, select a scheme → expect it applied, and one toast.
4. With two tabs open, select a scheme in one → expect the other to follow without a reload.

## 8. Client-side only (FR-022, FR-023, SC-007)

1. With the browser's network panel open, run sections 3, 5 and 6 → expect no new request containing scheme data.
2. `git diff --stat develop -- crates/` → expect nothing.

## 9. Desktop

Repeat sections 1, 3.4 and 6.1 in the desktop app on Windows, macOS and Linux. On Linux, check the color picker on WebKitGTK (research R6).
