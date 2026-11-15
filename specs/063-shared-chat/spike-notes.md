# Spike T004 notes (2026-11-08)

## 1. Blob Worker in a sandboxed srcdoc iframe on `file://`
Setup: `file://` page, `<iframe sandbox="allow-scripts" srcdoc=...>` using the same shape as the logic runner (`host.ts`/`runnerSource.ts`: relay creates a Blob URL Worker; worker calls `Function(...)`; reply relayed with `postMessage`). The frame carries its own CSP (`script-src 'unsafe-eval' 'unsafe-inline' blob:; worker-src blob:`). The *outer* page's CSP was varied. Script: Playwright 1.64.1.

Engines: Chromium 156.0.8078.4, Firefox 157.0, WebKit 27.2 (Playwright build on Linux; **not** Safari itself, so real Safari on macOS/iOS remains untested).

| Outer page CSP | Chromium | Firefox | WebKit |
|---|---|---|---|
| none | worker starts, eval OK | OK | OK |
| `script-src 'unsafe-inline'` only | worker blocked | blocked | blocked |
| + `worker-src blob:` | worker starts, **`Function` throws EvalError** | same | same |
| + `worker-src blob:` + `script-src 'unsafe-inline' 'unsafe-eval'` | OK | OK | OK |
| same, plus `frame-src` | OK (not needed) | OK | OK |

Findings: workers do start from `file://` in all three. A `srcdoc` frame inherits the parent's CSP, so the export page itself must allow `'unsafe-eval'` and `worker-src blob:` or the runner is dead (the frame's own CSP cannot loosen the parent's). `blob:` in the page's `script-src` and a `frame-src` are not needed (`default-src 'none'` did not block the srcdoc frame).

## 2. Single-file viewer size (`vite.viewer.config.ts`, vite 7.3.7, vite-plugin-singlefile 2.3.4)
| Build | Raw | gzip -9 |
|---|---|---|
| React only (the stub viewer) | 222,788 B (217.6 KiB) | 69,217 B |
| React + `three` + `@react-three/fiber` hello-world Canvas | 1,150,925 B (1.10 MiB) | 316,681 B |
| Difference (three) | ≈ 928 KB | ≈ 247 KB |

Real viewer (markdown, katex, highlight) will be larger than the stub; re-measure in T073. Build takes ~2-4 s. `publicDir:false` in the viewer config so `public/` files are not re-copied.

## 3. Playwright download sizes (disk, `~/.cache/ms-playwright`)
chromium 403M + headless_shell 273M + ffmpeg 5.1M, firefox 310M, webkit 269M: ≈ 1.26 GB on disk (downloads 111 MiB firefox, 96 MiB webkit, chromium similar). All installed and ran; none failed. Removable with `rm -rf ~/.cache/ms-playwright`.

## 4. Decisions
- **`run` stays enabled in exports** (works in the 3 engines when the CSP below is used). Cost: the export page allows `'unsafe-eval'`. Accepted because the page never renders raw HTML (R5.2), the data block is escaped (R6), and agent code only ever reaches `Function` inside the worker. Keep a flag/notice path anyway for engines where the worker fails to start (TP-78): if the worker errors, show the existing "no logic" notice. Real Safari must be checked by hand.
- **One template** for now: the 3D chunk adds ≈ 0.9 MB raw / 0.25 MB gzip, fine for a ≥ 5 MiB-budget file (TP-77). Build the two-template variant (T073) only if the real viewer measures above ≈ 2 MiB raw.
- **Export CSP (meta tag):**
  `default-src 'none'; script-src 'unsafe-inline' 'unsafe-eval'; style-src 'unsafe-inline'; img-src data: blob:; worker-src blob:; connect-src 'none'; form-action 'none'; base-uri 'none'`
- **Server CSP for `/share/*` HTML (T037):** `default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; media-src 'self' blob:; connect-src 'self'; worker-src blob:; object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'` plus `Referrer-Policy: no-referrer`. Same reason for `'unsafe-eval'` and `worker-src blob:` (srcdoc frame inherits it). Not browser-tested against the real SPA yet.
