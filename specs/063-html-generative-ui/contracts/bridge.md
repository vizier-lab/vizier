# Contract: the page bridge, v1 (`window.vizier`)

This is available to every page before its own scripts run.

```ts
interface VizierBridge {
  readonly version: 1
  readonly abilities: readonly ("state" | "send" | "files" | "worker")[]
  has(ability: string): boolean            // later phases add abilities; v1 pages keep working

  state: {
    get(): unknown                          // the page's saved state (null at first)
    set(value: unknown): void               // coalesced: at most one save per 500 ms
    subscribe(fn: (value: unknown) => void): () => void   // changes from other tabs
  }

  send(text: string, data?: unknown): Promise<void>
  // Becomes the person's next turn (client_event with page source).
  // Rejects with Error("no_interaction") unless within 10 s of a trusted click or key press in
  // the page, and with Error("rate_limited") at more than one per second.

  files: {
    url(name: string): Promise<string>     // the conversation's session file, as a data: URL
    // Rejects with Error("not_found: <available names>").
  }

  worker(source: ((input: any) => any) | string): {
    // source: a self-contained function (no closures, serialised with toString), or the CSS
    // selector of a <script type="vz-worker"> block whose code defines `self.run = (input) => ...`.
    run(input: unknown, options?: { timeoutMs?: number }): Promise<unknown>
    // Runs in a dedicated worker (its own thread: it cannot freeze the page or the WebUI).
    // One run at a time per worker; a second call queues. timeoutMs defaults to 10_000 and is
    // capped at 60_000; passing it terminates the worker and rejects with Error("timeout")
    // (the next run starts a fresh worker). A throw inside rejects with that error's message.
    terminate(): void
  }
}
```

**Host-side rules.** They are enforced outside the frame, so a page cannot get around them:
- Messages are accepted only from this frame's `contentWindow`.
- The interaction window, the send rate and state coalescing are enforced by the host.
- State is checked against `MAX_STATE_BYTES`.

**Theme.** The host posts `{type: "vz:theme", tokens: {...}, dark: bool}` on load and on every appearance change. The kit applies it to `:root` and fires `vizier:theme` on `window`.

**Workers.** A worker inherits the page's CSP, so it has no network access either. It does not get the kit or the bridge: it computes and returns.

**Errors.** The in-frame hooks post `error`, `unhandledrejection`, `console.error`, `securitypolicyviolation` and kit problems, plus a 250 ms heartbeat. The host uses them for verification and for the error indicator.

**Versioning.** A later bridge sets `version: 2` and adds abilities. Nothing in v1 is removed or changed in meaning.
