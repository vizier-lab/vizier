// The Vizier kit loaded inside every agent-made page (specs/063-html-generative-ui research R11).
//
// Built on its own (vite.kit.config.ts) as one classic script plus one stylesheet under
// `build/client/ui-kit/`, served next to the WebUI and listed in `ui-kit/manifest.json`, which the
// page host reads to know the hashed names. A page has an opaque origin, so nothing here may assume
// the WebUI's storage, token or DOM.
//
// The custom elements (`<vz-card>`, `<vz-stat>`, …) are registered here in later tasks.
import './base.css'

/** The kit's own version, so a page (or a later bridge) can tell what it was given. */
export const KIT_VERSION = 1

declare global {
    interface Window {
        vizierKit?: { version: number }
    }
}

window.vizierKit = { version: KIT_VERSION }
