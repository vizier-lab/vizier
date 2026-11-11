import { createRoot } from 'react-dom/client'
import { parseSnapshot } from '../lib/share/schema'
import { SnapshotView } from './SnapshotView'
import { DAMAGED, Neutral } from '../routes/share'

const el = document.getElementById('root')
if (el) {
    const tag = document.getElementById('vizier-snapshot')
    if (!tag || !tag.textContent) {
        createRoot(el).render(<Neutral message={DAMAGED} />)
    } else {
        try {
            const raw = JSON.parse(tag.textContent)
            const parsed = parseSnapshot(raw)
            if (parsed.ok) {
                if (parsed.snapshot.title) {
                    document.title = `${parsed.snapshot.title.slice(0, 120)} · Vizier`
                }
                createRoot(el).render(<SnapshotView snapshot={parsed.snapshot} />)
            } else {
                const message = parsed.reason === 'needs_newer' ? parsed.message : DAMAGED
                createRoot(el).render(<Neutral message={message} />)
            }
        } catch {
            createRoot(el).render(<Neutral message={DAMAGED} />)
        }
    }
}
