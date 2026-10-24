# Contract: the component catalogue (v1)

**Feature**: `026-generative-ui-terminal-tools` · implemented in `webui/app/lib/generativeUi/catalog.ts` (zod) and `registry.tsx` (design components). This file is the human-readable summary; the agent reads the generated version through `ui_components`.

Conventions: props are optional unless marked `*`. Any prop value may be an expression (`$state`, `$bindState`, `$template`, `$cond`, `$computed`, `$item`, `$index`). Closed enums only: there is no free-form style, class or HTML. Nothing loads from an external address: media are session files by name (or small `data:image/` graphics) and links are navigations the person chooses.

## 1. Components

### Layout
| Component | Props | Children | Events |
|---|---|---|---|
| `Stack` | `gap: none\|sm\|md\|lg`, `align: start\|center\|end\|stretch` | any | — |
| `Row` | `gap`, `align`, `justify: start\|center\|end\|between`, `wrap: bool` | any | — |
| `Grid` | `cols*: 1–12`, `gap`, `minCol: number` (px, responsive) | any (often with `repeat`) | — |
| `Card` | `title`, `subtitle`, `tone: default\|accent\|ok\|warn\|error` | any | — |
| `Tabs` | `tabs*: [{id,label}]`, `value: bind` | any (show/hide with `visible`) | `change` |
| `Fold` | `title*`, `open: bool` | any | — |
| `Divider` | — | none | — |

### Text
| Component | Props | Events |
|---|---|---|
| `Heading` | `text*`, `level: 1–4` | — |
| `Text` | `text*` (markdown subset: bold, italic, code, lists, links with the `Link` rules), `tone: normal\|muted\|ok\|warn\|error`, `size: sm\|md\|lg` | — |
| `Badge` | `text*`, `tone` | — |
| `Callout` | `text*`, `title`, `tone` | — |
| `Code` | `text*`, `lang` | — |

### Data
| Component | Props | Events |
|---|---|---|
| `Stat` | `label*`, `value*`, `delta`, `tone`, `format: number\|percent\|currency\|bytes\|duration` | — |
| `Table` | `columns*: [{key,label,align?,format?}]`, `rows*: [object]` (static or `$state`), `selectable: bool`, `maxRows` | `select` ({row,index}) |
| `KeyValue` | `items*: [{label,value}]` | — |
| `Progress` | `value*: 0–1`, `label`, `tone` | — |
| `Chart` | `kind*: bar\|line\|area\|pie`, `data*: [{label,value,series?}]`, `x`, `y`, `legend: bool`, `height: number` | `press` ({label,value}) |
| `Image` | `file` (a session file name) **or** `src` (`data:image/…`), `alt*`, `width`, `fit: contain\|cover` | `press` |

### Links and media
| Component | Props | Events |
|---|---|---|
| `Link` | `text*`, `href*` (`https:`, `http:`, `mailto:` only) **or** `file` (downloads a session file) | — |
| `Audio` | `file*`, `title`, `transcript` | — |
| `Video` | `file*`, `title`, `poster` (a session image), `height` | — |
| `FileCard` | `file*`, `title`, `preview: bool` (opens the standard preview dialog) | `press` |
| `Gallery` | `files*: [name]` (≤ 200), `cols`, `captions: [string]` | `press` ({file,index}) |

A `file` is the file's name in this conversation's session files (what `list_session_files` returns). Names are resolved by the WebUI at draw time; nothing is copied into the interface. Links open in a new tab (`noopener noreferrer nofollow`), show the host beside the label when it differs from the address, and refuse any other scheme. Audio and video never autoplay.

### Input (all take `label*`, `value: bind`, `disabled`, `checks: [{type,message}]`)
| Component | Extra props | Events |
|---|---|---|
| `TextInput` | `placeholder`, `multiline: bool`, `maxLength` | `change`, `submit` |
| `NumberInput` | `min`, `max`, `step` | `change` |
| `Slider` | `min*`, `max*`, `step` | `change` |
| `Select` | `options*: [string \| {value,label}]` | `change` |
| `Checkbox` | — (`value: bool`) | `change` |
| `Switch` | — (`value: bool`) | `change` |
| `RadioGroup` | `options*` | `change` |

### Action
| Component | Props | Events |
|---|---|---|
| `Button` | `label*`, `variant: primary\|secondary\|ghost\|danger`, `tone`, `disabled`, `selected: bool` | `press` |

### Surface (2D)
| Component | Props | Events |
|---|---|---|
| `Canvas2D` | `width*`, `height*` (logical units, scaled to fit), `background`, `shapes*: [Shape]` (static or `$state`) | `press` ({x,y,id?}), `move` (throttled) |

`Shape` = `{id?, kind: rect\|circle\|line\|path\|text\|image (`file`, x, y, w, h), …geometry, fill?, stroke?, strokeWidth?, text?, size?}` with colours from a closed token list (`ink`, `muted`, `accent`, `ok`, `warn`, `error`, `surface`, `series-1…6`) or `#rrggbb`.

### Surface (3D) — `Scene3D` is a container; its children are 3D components only
| Component | Props | Events |
|---|---|---|
| `Scene3D` | `height: number`, `background`, `camera: {position:[x,y,z], fov, lookAt}`, `shadows: bool` | — |
| `Box` / `Sphere` / `Cylinder` / `Plane` | `position`, `rotation`, `scale`, size props (`size`, `radius`, `height`, `segments`…), `color`, `flat: bool`, `wireframe: bool`, `id` | `press` ({id}) |
| `Group3D` | `position`, `rotation`, `scale` | — |
| `AmbientLight` / `DirectionalLight` / `PointLight` | `color`, `intensity`, `position` | — |
| `OrbitControls` | `autoRotate: bool`, `enableZoom: bool`, `target` | — |
| `Heightfield` | `size*: [w,d]`, `resolution*: n (≤192)`, `heights*: number[] (n×n, row-major)`, `heightScale`, `palette: [{at:0–1,color}]`, `flat: bool` (default true: low-poly shading), `wireframe` | `press` ({x,z,height}) |
| `Mesh` | `vertices*: number[]`, `indices: number[]`, `colors: number[]`, `flat: bool` | `press` ({id}) |

### Control
| Component | Props | Events |
|---|---|---|
| `Effect` | `intervalMs` (≥ 50; omit for no ticking), `running: bool` (default true) | `load` (once per draw), `tick` |

Renders nothing. Ticks pause off screen and in a hidden tab, and stop after 10 minutes without a person interaction.

## 2. Actions

| Action | Params | Effect |
|---|---|---|
| `setState` | `statePath*`, `value*` | json-render built-in |
| `pushState` / `removeState` | `statePath*`, `value` / `index` | json-render built-ins |
| `validateForm` | `statePath` | json-render built-in |
| `sendMessage` | `text*`, `data?` | Posts a client event as the person's message. Only from a chain started by a trusted DOM event; ≤ 1/s. |
| `run` | `fn*` (a `logic` name), `args?: any[]`, `into?: statePath` | Calls the function in the logic runner; writes the result to `into` (or discards it). On error: inline error on the interface, state unchanged. |

`on.<event>` accepts one binding or an ordered array (sequential; a failure stops the rest).

## 3. `$computed` functions (pure, no logic needed)

`add sub mul div mod min max abs round floor eq not and or len at concat join slice includes range` — e.g. `{"$computed":"mod","args":{"a":{"$index":true},"b":2}}`.

## 4. Limits

| What | Limit |
|---|---|
| Spec size / elements / depth | 256 KB / 1500 / 16 |
| `repeat` items rendered per container | 2048 |
| `logic` source total / params per function / functions | 64 KB / 8 / 32 |
| Logic call time / result size | 250 ms / 256 KB |
| `Heightfield.resolution` | 192 |
| `Canvas2D.shapes` | 5000 |
| Files per `Gallery` / distinct files per interface | 200 / 100 |
| Link `href` length | 2048 |
| Saved page state | 64 KB (excluding `_` keys) |
| `sendMessage` | 1/s per interface; text ≤ 4000 chars; data ≤ 64 KB |

## 5. Recipes (each is a stored, CI-validated spec returned by `ui_components(recipe=…)`)

| Recipe | Shows |
|---|---|
| `dashboard` | `Stack` → `Grid` of `Stat`s, a `Chart`, a `Table`; all static |
| `form` | inputs with `checks`, `validateForm`, `sendMessage` with the values |
| `tabs_and_filter` | `Tabs` + `visible`, a filter `Select` driving `repeat` over a list |
| `board_game` | 8×8 `Grid` + `repeat`, selection state, one `run step` per press, `$cond` highlights, a status `Text`, a "your move" `sendMessage` |
| `scene_3d` | `Scene3D` + `Heightfield` + lights + `OrbitControls`, three `Slider`s with `watch` → `run generate`, an `Effect load` |
| `canvas_2d` | `Canvas2D` with shapes from state, `press` → `run`, an `Effect tick` animation |

### Recipe sketch: `board_game` (abridged; the stored one is complete)

One state object `game`; one pure function `step(game, index)` that selects a piece (filling `legal`) or moves the selected one and passes the turn. Pressing a cell is a single local action.

```json
{
  "root": "page",
  "state": { "game": { "board": ["r","n","b","q","k","b","n","r","p","p","p","p","p","p","p","p",
                                 "","","","","","","","","","","","","","","","",
                                 "","","","","","","","","","","","","","","","",
                                 "P","P","P","P","P","P","P","P","R","N","B","Q","K","B","N","R"],
                       "turn": "w", "selected": null, "legal": [] } },
  "logic": {
    "step": { "params": ["game", "index"], "body": "/* ~90 lines: piece movement, check, turn; returns the new game object */" }
  },
  "elements": {
    "page":  { "type": "Stack", "props": { "gap": "md" }, "children": ["board", "status"] },
    "board": { "type": "Grid", "props": { "cols": 8, "gap": "none" },
               "repeat": { "statePath": "/game/board" }, "children": ["cell"] },
    "cell":  { "type": "Button",
               "props": {
                 "label": { "$item": "" },
                 "selected": { "$cond": { "$state": "/game/selected", "eq": { "$index": true } }, "$then": true, "$else": false },
                 "tone": { "$cond": { "$computed": "includes", "args": { "list": { "$state": "/game/legal" }, "value": { "$index": true } } },
                           "$then": "accent", "$else": "default" } },
               "on": { "press": { "action": "run",
                                  "params": { "fn": "step", "args": [{ "$state": "/game" }, { "$index": true }], "into": "/game" } } } },
    "status": { "type": "Text", "props": { "text": { "$template": "${/game/turn} to move" } } }
  }
}
```

A "Ask the agent for a move" `Button` can add `sendMessage { text: "Your move", data: { board: {"$state":"/game/board"} } }`, which starts a turn; the agent answers with text or a replacement interface.

### Recipe sketch: `scene_3d`

```json
{
  "root": "page",
  "state": { "seed": 7, "roughness": 0.55, "height": 3, "_heights": [] },
  "logic": { "generate": { "params": ["seed", "roughness", "n"], "body": "/* value-noise octaves → n*n numbers in 0..1 */" } },
  "elements": {
    "page":  { "type": "Stack", "children": ["scene", "controls", "boot"] },
    "scene": { "type": "Scene3D", "props": { "height": 420, "background": "#10141c", "camera": { "position": [40, 35, 40], "fov": 45 } },
               "children": ["sun", "ambient", "land", "orbit"] },
    "land":  { "type": "Heightfield", "props": { "size": [60, 60], "resolution": 48, "heights": { "$state": "/_heights" },
                                                 "heightScale": { "$state": "/height" }, "flat": true,
                                                 "palette": [{ "at": 0, "color": "#2b5d7a" }, { "at": 0.3, "color": "#4f8f4a" }, { "at": 0.7, "color": "#8a7d63" }, { "at": 1, "color": "#f2f4f7" }] } },
    "sun":     { "type": "DirectionalLight", "props": { "position": [20, 40, 10], "intensity": 1.1 } },
    "ambient": { "type": "AmbientLight", "props": { "intensity": 0.35 } },
    "orbit":   { "type": "OrbitControls", "props": { "autoRotate": false } },
    "controls": { "type": "Row", "children": ["seed", "rough"] },
    "seed":  { "type": "Slider", "props": { "label": "Seed", "min": 1, "max": 99, "step": 1, "value": { "$bindState": "/seed" } },
               "watch": { "/seed": { "action": "run", "params": { "fn": "generate", "args": [{ "$state": "/seed" }, { "$state": "/roughness" }, 48], "into": "/_heights" } } } },
    "rough": { "type": "Slider", "props": { "label": "Roughness", "min": 0, "max": 1, "step": 0.05, "value": { "$bindState": "/roughness" } },
               "watch": { "/roughness": { "action": "run", "params": { "fn": "generate", "args": [{ "$state": "/seed" }, { "$state": "/roughness" }, 48], "into": "/_heights" } } } },
    "boot": { "type": "Effect", "on": { "load": { "action": "run", "params": { "fn": "generate", "args": [{ "$state": "/seed" }, { "$state": "/roughness" }, 48], "into": "/_heights" } } } }
  }
}
```

## 6. Extending the catalogue

Add a zod entry (with `description` and an `example`) in `catalog.ts` and a component in `registry.tsx`. The index, detail output, `ui_render`'s `type` enum, validation and a test that every catalogue entry has a registered component and an example all follow from that one entry. No server change.
