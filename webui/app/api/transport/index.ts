// The only door to the engine: HTTP in a browser, Tauri commands in the desktop app.
// Chosen once; `transportReady` resolves when the desktop module (loaded lazily) is in place.
import { http } from './http'
import type { Transport } from './common'

export * from './common'

let current: Transport = http

const isDesktop = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

export const transportReady: Promise<void> = isDesktop
    ? import('./tauri').then((m) => {
          current = m.tauri
      })
    : Promise.resolve()

/** Delegates to the chosen implementation so importers can hold one stable object. */
export const transport: Transport = {
    get authMode() {
        return current.authMode
    },
    get desktop() {
        return current.desktop
    },
    request: (m, p, b, i) => current.request(m, p, b, i),
    upload: (p, f) => current.upload(p, f),
    fetchFile: (p, i) => current.fetchFile(p, i),
    assetUrl: (p) => current.assetUrl(p),
    saveFile: (p, f) => current.saveFile(p, f),
    openFile: (p, f) => current.openFile(p, f),
    openTopic: (t, o) => current.openTopic(t, o),
    notify: (n) => current.notify(n),
}
