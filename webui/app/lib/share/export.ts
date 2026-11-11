import { fetchViewerTemplate, transport } from '../../api/transport'
import { embedJson, escapeHtml } from './embed'
import type { Snapshot } from './schema'
import { exportFileName } from './slug'

export const VIZIER_SNAPSHOT_MARKER = '<!-- VIZIER_SNAPSHOT_MARKER -->'

export function buildOfflineHtml(template: string, snapshot: Snapshot): string {
    const escapedTitle = escapeHtml(snapshot.title ? `${snapshot.title.slice(0, 120)} · Vizier` : 'Vizier')
    const embeddedTag = `<script type="application/json" id="vizier-snapshot">${embedJson(snapshot)}</script>`

    let html = template.replace(/<title>.*?<\/title>/i, `<title>${escapedTitle}</title>`)
    if (html.includes(VIZIER_SNAPSHOT_MARKER)) {
        html = html.replace(VIZIER_SNAPSHOT_MARKER, embeddedTag)
    } else {
        html = html.replace('</body>', `${embeddedTag}</body>`)
    }
    return html
}

export async function exportOfflineSnapshot(snapshot: Snapshot): Promise<{ filename: string; blob: Blob }> {
    const template = await fetchViewerTemplate()
    const html = buildOfflineHtml(template, snapshot)
    const blob = new Blob([html], { type: 'text/html;charset=utf-8' })
    const filename = exportFileName(snapshot.title || 'conversation')

    if (transport.authMode === 'desktop') {
        await transport.saveText(html, filename)
    } else {
        const href = URL.createObjectURL(blob)
        const a = document.createElement('a')
        a.href = href
        a.download = filename
        a.click()
        setTimeout(() => URL.revokeObjectURL(href), 1000)
    }

    return { filename, blob }
}
