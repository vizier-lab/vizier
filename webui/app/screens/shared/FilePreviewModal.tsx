import { FilePreview, FilterTabs, Modal, Table, TableRow } from '@vizier/design'
import { useEffect, useMemo, useState } from 'react'
import { getToken } from '../../api/client'
import { Markdown } from './Markdown'

/** How much of a text file the preview reads. */
export const PREVIEW_BYTES = 256 * 1024

/** What the preview needs of a file, wherever it is kept (a conversation's upload, an artifact). */
export type PreviewFile = {
    key: string
    name: string
    mimeType: string
    size: number
    /** Absolute path of the bytes, fetched with the session's bearer token. */
    url: string
    /** "4 KB · sent by Archivist": shown under the name. */
    meta?: string
}

const url = (file: PreviewFile) => file.url

/** Minimal CSV parser: quotes, escaped quotes and CRLF. */
export function parseCsv(text: string): string[][] {
    const rows: string[][] = []
    let row: string[] = []
    let cell = ''
    let quoted = false
    for (let i = 0; i < text.length; i++) {
        const c = text[i]
        if (quoted) {
            if (c === '"' && text[i + 1] === '"') (cell += '"'), i++
            else if (c === '"') quoted = false
            else cell += c
        } else if (c === '"') quoted = true
        else if (c === ',') (row.push(cell), (cell = ''))
        else if (c === '\n' || c === '\r') {
            if (c === '\r' && text[i + 1] === '\n') i++
            row.push(cell)
            rows.push(row)
            row = []
            cell = ''
        } else cell += c
    }
    if (cell !== '' || row.length) (row.push(cell), rows.push(row))
    return rows
}

type Loaded = { text: string; partial: boolean; status: number }

async function readText(file: PreviewFile): Promise<Loaded> {
    const res = await fetch(url(file), { headers: { Authorization: `Bearer ${getToken()}`, Range: `bytes=0-${PREVIEW_BYTES - 1}` } })
    return { text: await res.text(), partial: res.status === 206 && file.size > PREVIEW_BYTES, status: res.status }
}

const kind = (f: PreviewFile) =>
    f.mimeType.startsWith('image/') ? 'image' : f.mimeType === 'application/pdf' || f.name.toLowerCase().endsWith('.pdf') ? 'pdf' : f.mimeType === 'text/csv' || f.name.endsWith('.csv') ? 'csv' : f.name.endsWith('.md') ? 'markdown' : f.mimeType.startsWith('text/') || f.mimeType === 'application/json' ? 'text' : 'none'

type Props = {
    files: PreviewFile[]
    index: number
    onIndex: (i: number) => void
    onClose: () => void
}

/** One preview for every file the WebUI keeps: CSV as a sortable table, text, markdown, images, PDFs (FR-018f). */
export function FilePreviewModal({ files, index, onIndex, onClose }: Props) {
    const file = files[index]
    const type = kind(file)
    const [loaded, setLoaded] = useState<Loaded | null>(null)
    const [blobUrl, setBlobUrl] = useState<string | null>(null)
    const [failed, setFailed] = useState(false)
    const [view, setView] = useState<'table' | 'raw'>('table')
    const [sort, setSort] = useState<{ col: number; dir: 1 | -1 } | null>(null)

    useEffect(() => {
        setLoaded(null)
        setBlobUrl(null)
        setFailed(false)
        setSort(null)
        let revoke: string | null = null
        let alive = true
        if (type === 'image' || type === 'pdf') {
            fetch(url(file), { headers: { Authorization: `Bearer ${getToken()}` } })
                .then((r) => {
                    if (!r.ok) throw new Error(String(r.status))
                    return r.blob()
                })
                .then((b) => {
                    if (!alive) return
                    // The stored type can be generic; a PDF only opens in the viewer as itself.
                    revoke = URL.createObjectURL(type === 'pdf' ? new Blob([b], { type: 'application/pdf' }) : b)
                    setBlobUrl(revoke)
                })
                .catch(() => alive && setFailed(true))
        } else if (type !== 'none') {
            readText(file).then((l) => alive && setLoaded(l)).catch(() => alive && setLoaded({ text: '', partial: false, status: 0 }))
        }
        return () => {
            alive = false
            if (revoke) URL.revokeObjectURL(revoke)
        }
    }, [file.key]) // eslint-disable-line react-hooks/exhaustive-deps

    useEffect(() => {
        const onKey = (e: KeyboardEvent) => {
            if (e.key === 'ArrowLeft' && index > 0) onIndex(index - 1)
            if (e.key === 'ArrowRight' && index < files.length - 1) onIndex(index + 1)
            if (e.key === 'Escape') onClose()
        }
        window.addEventListener('keydown', onKey)
        return () => window.removeEventListener('keydown', onKey)
    }, [index, files.length, onIndex, onClose])

    const rows = useMemo(() => (type === 'csv' && loaded ? parseCsv(loaded.text) : []), [type, loaded])
    const header = rows[0] ?? []
    const body = useMemo(() => {
        const data = rows.slice(1)
        if (!sort) return data
        return [...data].sort((a, b) => {
            const x = a[sort.col] ?? '', y = b[sort.col] ?? ''
            const nx = Number(x), ny = Number(y)
            return (Number.isNaN(nx) || Number.isNaN(ny) ? x.localeCompare(y) : nx - ny) * sort.dir
        })
    }, [rows, sort])

    const download = async () => {
        const res = await fetch(url(file), { headers: { Authorization: `Bearer ${getToken()}` } })
        const href = URL.createObjectURL(await res.blob())
        const a = document.createElement('a')
        a.href = href
        a.download = file.name
        a.click()
        URL.revokeObjectURL(href)
    }

    const note = loaded?.partial ? `showing the first ${Math.round(PREVIEW_BYTES / 1024)} KB` : null

    return (
        <Modal open label={`Preview of ${file.name}`} onClose={onClose}>
        <FilePreview
            className="h-[min(720px,88vh)] w-[min(960px,92vw)]"
            look="dialog"
            name={file.name}
            meta={file.meta}
            viewBar={
                type === 'csv' ? (
                    <div className="flex items-center gap-12">
                        <FilterTabs
                            aria-label="View"
                            value={view}
                            onChange={setView}
                            options={[{ value: 'table', label: 'Table' }, { value: 'raw', label: 'Raw' }]}
                        />
                        {note && <span className="type-mono-xs text-faint">{note}</span>}
                    </div>
                ) : note ? (
                    <span className="type-mono-xs text-faint">{note}</span>
                ) : undefined
            }
            files={files.map((f, i) => ({ name: f.name, current: i === index, onSelect: () => onIndex(i) }))}
            onPrevious={index > 0 ? () => onIndex(index - 1) : undefined}
            onNext={index < files.length - 1 ? () => onIndex(index + 1) : undefined}
            onDownload={() => void download()}
            onOpenInTab={() => window.open(url(file) + `?token=${getToken()}`, '_blank', 'noopener')}
            onClose={onClose}
        >
            {failed ? (
                <p className="type-small text-failed-text" role="alert">This file couldn’t be loaded. It may have been removed.</p>
            ) : type === 'image' ? (
                blobUrl && <img src={blobUrl} alt={file.name} className="max-h-[calc(min(720px,88vh)-130px)] max-w-full object-contain" />
            ) : type === 'pdf' ? (
                blobUrl ? (
                    <iframe src={blobUrl} title={file.name} className="h-[calc(min(720px,88vh)-130px)] w-[min(880px,84vw)] rounded-sm border border-border bg-surface" />
                ) : (
                    <p className="type-small text-muted">Loading…</p>
                )
            ) : loaded && loaded.status >= 400 ? (
                <p className="type-small text-failed-text" role="alert">This file couldn’t be loaded. It may have been removed.</p>
            ) : type === 'none' ? (
                <p className="type-small text-muted">No preview for this type. Download it to open it.</p>
            ) : !loaded ? (
                <p className="type-small text-muted">Loading…</p>
            ) : type === 'csv' && view === 'table' ? (
                <Table
                    dense
                    label={file.name}
                    columns={header.map((label, i) => ({ label, sorted: sort?.col === i ? (sort.dir === 1 ? ('asc' as const) : ('desc' as const)) : undefined, onSort: () => setSort((s) => (s?.col === i ? (s.dir === 1 ? { col: i, dir: -1 } : null) : { col: i, dir: 1 })) }))}
                >
                    {body.map((r, i) => (
                        <TableRow key={i} cells={header.map((_, c) => r[c] ?? '')} />
                    ))}
                </Table>
            ) : type === 'markdown' ? (
                <Markdown size="document">{loaded.text}</Markdown>
            ) : (
                <pre className="whitespace-pre-wrap type-mono-sm text-fg">{loaded.text}</pre>
            )}
        </FilePreview>
        </Modal>
    )
}
