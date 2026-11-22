// One HTML page in a shared copy (spec 063): the document the agent drew, in a frame of its own.
//
// The frame is `srcdoc`, so the link and the exported file need no server, and it is `sandbox`
// without `allow-same-origin`: an opaque origin that cannot reach this window's storage, cookies or
// DOM. The policy is put into the document itself (a `srcdoc` frame inherits nothing from this page),
// and it allows only what the page already carries: its own inline script and style. Nothing is
// fetched: no `https:` images, no fonts, no connections. A page that wants its state saved or a
// message sent finds no host here; `vizier.*` calls resolve to nothing, as `actions.ts` says.
import { InteractivePage as Frame } from '@vizier/design'
import { useEffect, useMemo, useRef, useState } from 'react'
import type { PagePart } from '../lib/share/schema'
import { parseFrameMessage } from '../lib/uiPage/host'

/**
 * The frame's policy. `script-src` and `style-src` are inline because a page is one document with
 * its script in it; `blob:` is for the kit's library loader and workers. `default-src 'none'` and
 * `connect-src 'none'` keep every request out, including the page's own `<img src="https://…">`,
 * which would otherwise be a way to tell that the copy was opened.
 */
export const PAGE_CSP =
    "default-src 'none'; script-src 'unsafe-inline' blob:; style-src 'unsafe-inline'; img-src data: blob:; media-src data: blob:; font-src data:; connect-src 'none'; worker-src blob:; frame-src 'none'; form-action 'none'; base-uri 'none'"

const CSP_META = `<meta http-equiv="Content-Security-Policy" content="${PAGE_CSP}">`
/** The shortest a frame is drawn, and the tallest it may grow to, in px. */
const MIN_HEIGHT = 240
const MAX_HEIGHT = 720

/**
 * The page's document with this policy as its first `head` child. Ours is always added: policies
 * combine, so one the page brought along can only narrow what ours allows, never widen it.
 */
export function pageDocument(html: string): string {
    const head = /<head(?:\s[^>]*)?>/i.exec(html)
    if (head) {
        const at = head.index + head[0].length
        return html.slice(0, at) + CSP_META + html.slice(at)
    }
    // A page without a head: keep a leading doctype where it is, so the frame is not drawn in quirks mode.
    const doctype = /^\s*<!doctype[^>]*>/i.exec(html)
    if (doctype) {
        const at = doctype[0].length
        return html.slice(0, at) + CSP_META + html.slice(at)
    }
    return CSP_META + html
}

export function SharedPage({ part }: { part: PagePart }) {
    const ref = useRef<HTMLIFrameElement>(null)
    const [height, setHeight] = useState(MIN_HEIGHT)
    const doc = useMemo(() => pageDocument(part.html), [part.html])
    const title = part.title || 'Interactive page'

    // The kit's runtime reports its height (`{ vz: 1, type: 'height', px }`), the same message the
    // chat's host reads. Only this frame's window is believed, and the height is kept in bounds.
    useEffect(() => {
        const onMessage = (event: MessageEvent) => {
            if (event.source !== ref.current?.contentWindow) return
            const message = parseFrameMessage(event.data)
            if (message?.type === 'height') setHeight(Math.min(MAX_HEIGHT, Math.max(MIN_HEIGHT, message.px)))
        }
        window.addEventListener('message', onMessage)
        return () => window.removeEventListener('message', onMessage)
    }, [])

    return (
        <Frame
            title={title}
            label={part.version ? `v${part.version} · read-only` : 'read-only'}
            className="min-w-0 max-w-full"
            data-testid="shared-page"
        >
            <div className="min-w-0 max-w-full overflow-hidden">
                <iframe
                    ref={ref}
                    title={title}
                    sandbox="allow-scripts"
                    referrerPolicy="no-referrer"
                    srcDoc={doc}
                    className="block w-full max-w-full border-0 bg-transparent"
                    style={{ height }}
                />
            </div>
        </Frame>
    )
}
