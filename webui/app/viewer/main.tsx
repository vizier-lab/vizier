// The entry of the single-file viewer (`viewer.html`, the offline `.vizier.html`). It reads the one
// embedded snapshot, validates it as hostile input and draws it. It imports nothing from `routes/`
// or `api/`: the offline file has no server to talk to, and the bundle should not carry a client.
import { createRoot } from 'react-dom/client'
import { parseSnapshot } from '../lib/share/schema'
import { DAMAGED, NeutralNotice, OFFLINE_BANNER } from './Neutral'
import { SnapshotView } from './SnapshotView'

const TAG_ID = 'vizier-snapshot'

const el = document.getElementById('root')
if (el) {
    const root = createRoot(el)
    const damaged = (message = DAMAGED) => root.render(<NeutralNotice message={message} />)
    // Exactly one snapshot tag. A second one means the file was edited, or content got out of its tag.
    const tags = document.querySelectorAll(`[id="${TAG_ID}"]`)
    const tag = tags.length === 1 ? tags[0] : null
    if (!tag || tag.tagName !== 'SCRIPT' || !tag.textContent) {
        damaged()
    } else {
        try {
            const parsed = parseSnapshot(JSON.parse(tag.textContent))
            if (parsed.ok) {
                if (parsed.snapshot.title) document.title = `${parsed.snapshot.title.slice(0, 120)} · Vizier`
                root.render(<SnapshotView snapshot={parsed.snapshot} banner={OFFLINE_BANNER} />)
            } else {
                damaged(parsed.reason === 'needs_newer' ? parsed.message : DAMAGED)
            }
        } catch {
            damaged()
        }
    }
}
