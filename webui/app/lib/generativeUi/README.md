# Read-only json-render renderer (legacy, #82)

This folder draws pages saved by the json-render generative UI of spec 026 (the client tool
`ui` + `render`). Since spec 063 the agent draws pages with the built-in `ui_render` tool (HTML
plus the Vizier kit, `app/kit/`), and the WebUI no longer declares `render` or `components`.

What stays is what drawing an old saved call needs: the catalogue, its renderers, validation
(a saved spec is re-checked before it is drawn), page state, watches, actions, links, media and
the logic worker. `recipes/` stays as the renderer's test fixtures. The agent-facing reference
and the tool declarations were removed.

Remove this folder, `screens/chat/client/GenerativeUi.tsx` and their registration in
`lib/clientTools/builtin.tsx` with the other legacy shims before 1.0.0 (#82).
