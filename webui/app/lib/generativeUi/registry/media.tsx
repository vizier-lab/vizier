import { FilePeek, ImageGallery } from '@vizier/design'
import type { ComponentRenderProps } from '@json-render/react'
import { useState } from 'react'
import { getToken } from '../../../api/client'
import type { SessionFileRecord } from '../../../api/frames'
import { Audio as AudioPlayer, Video as VideoPlayer } from '../../../screens/chat/ReplyMedia'
import { FilePreviewDialog } from '../../../screens/chat/FilePreviewDialog'
import { fileUrl, findFile, IMAGE_MIME } from '../media'
import { FileLink, SafeLink } from './anchor'
import { useSessionFiles } from './mediaContext'
import { useEmitter } from './shared'

type P = Record<string, unknown>

/** What stands where a file is missing: this spot only, the rest of the interface is unaffected. */
function Missing({ name }: { name: string }) {
    return (
        <p role="img" aria-label={`${name} is not available`} className="rounded-md border border-dashed border-border-strong px-12 py-8 type-small text-faint">
            File not available: {name}
        </p>
    )
}

export function LinkC({ element }: ComponentRenderProps<P>) {
    const p = element.props
    const files = useSessionFiles()
    const text = String(p.text ?? '')
    if (typeof p.file === 'string') {
        const file = findFile(files, p.file)
        return file ? <FileLink file={file} label={text} /> : <Missing name={p.file} />
    }
    return <SafeLink href={String(p.href ?? '')} label={text} />
}

export function ImageC({ element, emit }: ComponentRenderProps<P>) {
    const p = element.props
    const files = useSessionFiles()
    const send = useEmitter(emit)
    const file = typeof p.file === 'string' ? findFile(files, p.file) : undefined
    const src = typeof p.src === 'string' && p.src.startsWith('data:image/') ? p.src : file ? fileUrl(file, getToken()) : null
    if (!src) return <Missing name={typeof p.file === 'string' ? p.file : 'image'} />
    return (
        // eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-noninteractive-element-interactions
        <img
            src={src}
            alt={String(p.alt ?? '')}
            referrerPolicy="no-referrer"
            loading="lazy"
            onClick={(e) => send('press', e.nativeEvent)}
            className="rounded-md"
            style={{ maxWidth: typeof p.width === 'number' ? p.width : '100%', objectFit: p.fit === 'cover' ? 'cover' : 'contain' }}
        />
    )
}

export function AudioC({ element }: ComponentRenderProps<P>) {
    const p = element.props
    const file = findFile(useSessionFiles(), String(p.file ?? ''))
    if (!file) return <Missing name={String(p.file ?? 'audio')} />
    return (
        <div className="flex flex-col gap-4">
            {typeof p.title === 'string' && <p className="type-ui-medium text-soft">{p.title}</p>}
            <AudioPlayer src={fileUrl(file, getToken())} transcript={typeof p.transcript === 'string' ? p.transcript : undefined} />
        </div>
    )
}

export function VideoC({ element }: ComponentRenderProps<P>) {
    const p = element.props
    const file = findFile(useSessionFiles(), String(p.file ?? ''))
    if (!file) return <Missing name={String(p.file ?? 'video')} />
    return (
        <div className="flex flex-col gap-4">
            {typeof p.title === 'string' && <p className="type-ui-medium text-soft">{p.title}</p>}
            <VideoPlayer a={{ filename: typeof p.title === 'string' ? p.title : file.filename }} src={fileUrl(file, getToken())} />
        </div>
    )
}

export function FileCardC({ element, emit }: ComponentRenderProps<P>) {
    const p = element.props
    const files = useSessionFiles()
    const file = findFile(files, String(p.file ?? ''))
    const send = useEmitter(emit)
    const [open, setOpen] = useState(false)
    if (!file) return <Missing name={String(p.file ?? 'file')} />
    const download = () => window.open(fileUrl(file, getToken()), '_blank', 'noopener,noreferrer')
    return (
        <>
            <FilePeek
                name={typeof p.title === 'string' ? p.title : file.filename}
                meta={file.mime_type}
                action={
                    p.preview === true
                        ? { label: 'Open', onClick: () => (setOpen(true), send('press', { isTrusted: true })) }
                        : { label: 'Download', onClick: () => (download(), send('press', { isTrusted: true })) }
                }
            />
            {open && <FilePreviewDialog files={[file]} index={0} onIndex={() => {}} onClose={() => setOpen(false)} />}
        </>
    )
}

export function GalleryC({ element, emit }: ComponentRenderProps<P>) {
    const p = element.props
    const files = useSessionFiles()
    const send = useEmitter(emit)
    const [open, setOpen] = useState<number | null>(null)
    const names = (Array.isArray(p.files) ? p.files : []) as string[]
    const found = names.map((name) => ({ name, file: findFile(files, name) }))
    const shown = found.filter((x): x is { name: string; file: SessionFileRecord } => !!x.file && IMAGE_MIME.test(x.file.mime_type))
    const missing = found.filter((x) => !x.file)
    const token = getToken()
    return (
        <div className="flex flex-col gap-8">
            <ImageGallery
                images={shown.map((x) => ({ src: fileUrl(x.file, token), name: x.name }))}
                onOpen={(index) => {
                    setOpen(index)
                    send('press', { isTrusted: true }, { file: shown[index]?.name, index })
                }}
            />
            {missing.map((x) => (
                <Missing key={x.name} name={x.name} />
            ))}
            {open !== null && shown[open] && <FilePreviewDialog files={shown.map((x) => x.file)} index={open} onIndex={setOpen} onClose={() => setOpen(null)} />}
        </div>
    )
}
