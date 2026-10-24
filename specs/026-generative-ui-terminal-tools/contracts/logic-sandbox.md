# Contract: the logic sandbox

**Feature**: `026-generative-ui-terminal-tools` · spec FR-029…FR-032 · implemented in `webui/app/lib/generativeUi/logic/`

Agent-written functions are the only agent-written code that runs on the person's device. This is the whole surface.

## 1. What a function is

`logic: { "<name>": { "params": ["a","b"], "body": "return a + b;" } }` is compiled as `new Function(...params, body)` inside the runner. It is pure by convention and by isolation:

- Inputs: JSON values (the action's resolved `args`). Output: a JSON-serialisable value (`undefined` becomes `null`).
- Available: the JavaScript language and its standard built-ins (`Math`, `Array`, `Object`, `JSON`, typed arrays, `Map`, `Set`, `BigInt`, `Date`, `Intl`).
- Not available (absent or blocked): the DOM, `window.parent`/`top`, cookies, `localStorage`/IndexedDB, `fetch`/`XMLHttpRequest`/`WebSocket`/`EventSource`, `importScripts`/`import()`, `postMessage` to anything but the host, timers beyond the call (`setTimeout`/`setInterval` are removed), workers.
- Functions can call each other by name through a provided `fns` object (`fns.legalMoves(board, i)`); depth ≤ 32.

## 2. Isolation

| Layer | Rule |
|---|---|
| Container | `<iframe sandbox="allow-scripts" srcdoc=… referrerpolicy="no-referrer">`: no `allow-same-origin`, so an opaque origin; no forms, popups, modals, top navigation, downloads. |
| Thread | The frame holds only a relay. The functions run in a **Worker** the relay starts from a blob, so a loop that never ends burns the worker's thread and nothing else. (Measured: in Chromium a script in a sandboxed frame can share the page's thread, and an infinite loop then froze the page and the host's timer; with the worker the page kept running.) Destroying the frame ends the worker. |
| CSP (meta, first in the document) | `default-src 'none'; script-src 'unsafe-eval' 'unsafe-inline' blob:; worker-src blob:; connect-src 'none'; img-src 'none'; style-src 'none'` — `unsafe-eval` for `new Function` and a blob worker, and nothing else. The worker inherits it, so it has no network either. |
| Messages | The host accepts messages only from this frame's `contentWindow`; the relay accepts only from `parent` and passes them to the worker. Each carries a call id; unknown ids are dropped. |
| Data | Arguments and results cross as structured-clone plain data. Functions, DOM nodes and class instances are rejected. Args ≤ 256 KB, results ≤ 256 KB. |

## 3. Limits and failure

| Limit | Value | On breach |
|---|---|---|
| Wall-clock per call | 250 ms (host timer) | Frame destroyed and recreated; rejects `logic_timeout`. |
| Queue | one call at a time, FIFO, ≤ 16 waiting | Extra calls reject `logic_busy`. |
| Result size | 256 KB | Rejects `logic_result_too_large`. |
| Thrown error | message only (first 300 chars) | Rejects `logic_error`. |

A rejected `run` leaves the target state path unchanged, stops the rest of its action chain, and shows a small inline error under the interface (`<fn>: <message>`), cleared by the next successful call. It is also `console.warn`ed with the function name. It never throws into React and never blocks other interfaces (each has its own runner).

## 4. Lifecycle

Created on the first `run` of an interface (not on draw), reused for the interface's life, destroyed on unmount, when the interface is scrolled far off screen for 60 s (recreated on demand), and after a timeout. A page reload recompiles from the saved spec; no logic state survives (functions are pure), so there is nothing to persist.

## 5. Timed actions and watch

- `Effect.tick` fires `on.tick` every `intervalMs` (≥ 50 ms) while the interface is on screen, the tab is visible and `running` is true; one tick at a time (a tick that is still running skips the next).
- `watch` fires only on changes made after the first draw, never from the initial state, as in json-render. A `run` writing the same path it watches does not re-trigger itself (guard: an action chain started by a `watch` ignores `watch` triggers on its own writes).
- Chain depth ≤ 8 actions triggered by one event, to stop `watch` loops between paths.

## 6. Trust and `sendMessage`

`watch`, `Effect` and logic results run with `trusted = false`. `sendMessage` requires `trusted = true`, which only an action chain begun by a real (`isTrusted`) DOM event from a catalogue component carries. A logic function cannot send a message, whatever it returns.

## 7. What this does not defend against

A function can burn 250 ms of CPU per call, repeatedly (bounded by the call queue and, for ticks, the 50 ms minimum interval and the 10-minute idle stop). A person who dislikes an interface can collapse it; closing the conversation destroys the runner. Memory is bounded by the iframe's own lifetime, not by a per-call cap (research R9, "Why not QuickJS": revisit if this proves insufficient).
