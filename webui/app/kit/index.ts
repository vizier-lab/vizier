// The Vizier kit loaded inside agent-made pages that use `<vz-*>` elements (specs/063-html-generative-ui
// research R11, tasks T052 and T055).
//
// Built on its own (vite.kit.config.ts) as one classic script plus `base.css`; the server inlines
// both into a page's frame document (the stylesheet always, this script only when the page has a
// `<vz-` tag). A page has an opaque origin, so nothing here may assume the WebUI's storage, token
// or DOM.
import './base.css'
import elementsCss from './elements.css?inline'
import './elements/index'
import { elementStyles } from './elements/values'

/** The kit's own version, so a page (or a later bridge) can tell what it was given. */
export const KIT_VERSION = 1

declare global {
    interface Window {
        vizierKit?: { version: number }
    }
}

const style = document.createElement('style')
style.dataset.vzKit = 'elements'
style.textContent = elementStyles(elementsCss)
// After the kit's base stylesheet, so the layer order it declares (vz-base between Tailwind's
// base and utilities) is the one in force.
document.head.appendChild(style)

window.vizierKit = { version: KIT_VERSION }
