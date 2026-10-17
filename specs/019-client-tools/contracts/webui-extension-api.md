# Contract: WebUI Client Tool Extension API

**Feature**: `019-client-tools`

The WebUI's own client tools, `render_interactive_ui` first, are registered through this API. A WebUI extension uses the same API to add its own. It lives in `webui/app/lib/clientTools.ts`.

## Registering a tool

```ts
export interface ClientToolDefinition {
  name: string                    // protocol §1 rules
  category?: string               // default 'client'
  description: string
  input_schema: JsonSchemaObject
  output_schema?: JsonSchemaObject
  timeout_secs?: number
}

export interface ClientCallContext {
  callId: string
  agentId: string
  topicId: string
  /** Start an agent turn from this call (protocol §5). Throttled by the caller's own rules. */
  sendEvent(text: string, data?: unknown): void
  /** Save this call's state (protocol §6). */
  saveState(state: unknown): void
}

export interface ClientCallView {
  callId: string
  name: string            // resolved name
  category: string
  tool: string
  args: Record<string, unknown>
  outcome?: ClientCallOutcome
  state?: unknown
  live: boolean           // true while its turn is streaming
}

export interface ClientToolRegistration {
  definition: ClientToolDefinition
  /** Runs once, on the connection that declared the tool, when the agent calls it. */
  handle(args: Record<string, unknown>, ctx: ClientCallContext): Promise<unknown> | unknown
  /** Draws a call: live, on reload, in other tabs and browsers. Optional. */
  render?: (call: ClientCallView, ctx: ClientCallContext) => React.ReactNode
  /** Draw only once the call's turn has ended (answered, failed or aborted). */
  renderAfterTurn?: boolean
  /** Of this tool's calls in one turn, draw only the last. */
  latestPerTurn?: boolean
}

export function registerClientTool(registration: ClientToolRegistration): () => void
export function registerClientCategory(category: { name: string; description?: string }): () => void
```

**Behaviour**:

- Every registered definition is declared on every request the WebUI sends (`connectionStore.sendMessage` adds `client_tools` / `client_categories`).
- Registering a second tool with the same `(category, name)` replaces the first. The returned function unregisters.
- `handle`'s return value is sent as `ok`. A thrown `Error` is sent as `error` with its message. A `handle` that never settles is ended by the server's timeout.
- A call for a tool with no registration is answered `error: "this client does not provide <tool>"`.
- `render` is used for `turn.clientCalls` entries whose `(category, tool)` matches. Calls with no matching renderer render as `<ClientCallChip>` (tool name, status, collapsed arguments).
- `renderAfterTurn` hides the call while its turn is still running; `latestPerTurn` hides all but the turn's last call of the tool (research R19).
- A call whose `outcome` is `deferred` is delivered to this WebUI when it next opens the conversation, and `handle` runs then.

## The built-in: `render_interactive_ui`

```ts
registerClientTool({
  definition: {
    name: 'render_interactive_ui',
    category: 'ui',
    deferrable: true,
    description: 'Show an interactive HTML page inside your reply in the WebUI …',
    input_schema: {
      type: 'object',
      properties: {
        title: { type: 'string', description: 'Short title, shown to screen readers and in the collapsed view' },
        html:  { type: 'string', description: 'A complete, self-contained HTML document (inline CSS/JS, data: images only, no network)' },
      },
      required: ['title', 'html'],
    },
  },
  handle: (args, ctx) => {
    if (byteLength(args.html) > 256 * 1024) throw new Error('html is larger than 256 KB; make the page smaller')
    return { ui_id: ctx.callId, status: 'rendered' }
  },
  render: (call, ctx) => <InteractivePage call={call} ctx={ctx} />,
  renderAfterTurn: true,   // the page appears when the turn ends
  latestPerTurn: true,     // one page per reply bubble: a later call replaces it
})
registerClientCategory({ name: 'ui', description: 'Show interactive pages in the conversation' })
```

The tool description tells the agent:
- what the bridge offers: `vizier.send(text, data?)` is "the person clicked something, start a turn"; `vizier.state`, `vizier.saveState(v)`, `vizier.onState(cb)`;
- that the page has no network and no access to the WebUI;
- that sends only work after the person interacts with the page;
- to keep pages small;
- that the page appears once the reply is complete, a reply shows one page, and calling again in the same reply replaces it.

**Theme**: each page's base style defines `--bg`, `--surface`, `--border`, `--text`, `--muted`, `--accent`, `--error`, read from the WebUI's own variables (`--background`, `--surface`, `--border`, `--text-primary`, `--text-secondary`, `--accent-primary`, `--color-error`) when the page is drawn, with the theme's values as fallbacks. Text, links, buttons and inputs use them by default; the page's own styles override them. Downloads and PDFs carry the same variables.

**Page controls**: on hover or keyboard focus, a page shows three buttons: expand (the page covers the chat area — messages and input — pinned over its rectangle so the frame is not reloaded; the app's sidebar and header stay visible; Esc or the button collapses it) download (`<title-slug>.html`: the theme and background the page was drawn with, the same CSP, the latest saved state as `vizier.state`, and a `vizier` whose `send` and `onState` do nothing), and save as PDF (the same document printed with `print-color-adjust: exact` and no page margin, so backgrounds are kept, from a short-lived hidden frame sandboxed `allow-scripts allow-modals` with the page's own `alert`/`confirm`/`prompt`/`print` disabled first; the frame is removed when the print dialog closes). Each page send is announced with an info toast, shown above an expanded page.

## The page bridge (inside the iframe)

Injected before the agent's HTML:

```ts
window.vizier = {
  state: <initial state or null>,
  send(text: string, data?: unknown): void,     // forwarded only after interaction, ≤ 1/s
  saveState(value: unknown): void,               // coalesced to the latest value per 500 ms
  onState(cb: (value: unknown) => void): void,   // called when another tab saves
}
```

The bridge posts `{ type: 'vizier:height', height }` from a `ResizeObserver`, and `{ type: 'vizier:send' | 'vizier:state', … }` for the calls above. The host (`InteractivePage`) accepts messages only from its own iframe's `contentWindow`. It answers with `{ type: 'vizier:state', value }` when the call's state changes elsewhere.

## Isolation guarantees (SC-004 test list)

From inside a page, each of these must fail:

1. reading `localStorage`, `sessionStorage`, `document.cookie`, or `indexedDB` (opaque origin);
2. `window.parent.document`, `window.top.location`, or any other parent DOM access (`SecurityError`);
3. `fetch`, `XMLHttpRequest`, `WebSocket`, `new Image().src = 'https://…'`, `<link>`/`<script src>` to a URL (CSP);
4. `alert`, `confirm`, `prompt`, `window.open`, form submission, top navigation (sandbox flags);
5. `vizier.send` on load or from a timer with no interaction (host gate);
6. `vizier.send` 10 times in a burst after one click (host throttle: one forwarded per second).
