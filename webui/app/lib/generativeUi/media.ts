// Session files by name: how an interface refers to a file of its conversation (research R12).
// Nothing about a file is stored in the interface but its name; the record, and the address it is
// served from, are looked up when the interface is drawn.
import type { SessionFileRecord } from '../../api/frames'

export const IMAGE_MIME = /^image\//

/** The file called `name`; of several with that name, the newest. */
export function findFile(files: readonly SessionFileRecord[], name: string): SessionFileRecord | undefined {
    return files.filter((f) => f.filename === name).sort((a, b) => (a.added_at < b.added_at ? 1 : a.added_at > b.added_at ? -1 : 0))[0]
}

/** Where a file is served from. The file route takes the token as a query for elements that cannot send headers. */
export function fileUrl(file: SessionFileRecord, token: string | null): string {
    return `/api/v1/files/${encodeURIComponent(file.file_id)}${token ? `?token=${encodeURIComponent(token)}` : ''}`
}

/** "forecast.png (image/png), report.pdf (application/pdf)" for an error that lists what exists. */
export function availableFiles(files: readonly SessionFileRecord[]): string {
    if (files.length === 0) return 'This conversation has no files yet.'
    const seen = new Set<string>()
    const list = files.filter((f) => !seen.has(f.filename) && seen.add(f.filename)).map((f) => `${f.filename} (${f.mime_type})`)
    return `Available: ${list.slice(0, 20).join(', ')}${list.length > 20 ? `, and ${list.length - 20} more` : ''}.`
}

/** The names of files a spec's elements refer to, with the path of each reference. */
export function fileReferences(elements: Record<string, { type?: string; props?: Record<string, unknown> }>): { path: string; name: string }[] {
    const out: { path: string; name: string }[] = []
    const literal = (value: unknown): value is string => typeof value === 'string'
    for (const [key, element] of Object.entries(elements)) {
        const props = element?.props ?? {}
        const at = (prop: string) => `elements.${key}.props.${prop}`
        for (const prop of ['file', 'poster'] as const) if (literal(props[prop])) out.push({ path: at(prop), name: props[prop] as string })
        if (Array.isArray(props.files)) props.files.forEach((name, i) => literal(name) && out.push({ path: `${at('files')}[${i}]`, name }))
        // Canvas2D image shapes.
        if (element?.type === 'Canvas2D' && Array.isArray(props.shapes)) {
            props.shapes.forEach((shape, i) => {
                if (shape && typeof shape === 'object' && (shape as { kind?: string }).kind === 'image' && literal((shape as { file?: unknown }).file)) {
                    out.push({ path: `${at('shapes')}[${i}].file`, name: (shape as { file: string }).file })
                }
            })
        }
    }
    return out
}
