import { AudioPlayer, FilePeek, ImageGallery, VideoPlayer } from '@vizier/design'
import { useRef, useState } from 'react'
import type { VizierAttachment } from '../../api/frames'
import { artifactContentUrl } from '../../api/endpoints/artifacts'
import { relativeToApi, transport } from '../../api/transport'
import { downloadFile } from '../../lib/download'
import { assetPath } from '../../lib/fileUrls'
import { FilePreviewModal, type PreviewFile } from '../shared/FilePreviewModal'

const EXT = (name: string) => name.split('.').pop()?.toLowerCase() ?? ''
const IMAGE = new Set(['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'avif'])
const AUDIO = new Set(['mp3', 'wav', 'ogg', 'oga', 'm4a', 'webm', 'opus', 'flac'])
const VIDEO = new Set(['mp4', 'mov', 'mkv', 'm4v'])
const MIME: Record<string, string> = { csv: 'text/csv', md: 'text/markdown', txt: 'text/plain', json: 'application/json', pdf: 'application/pdf' }

export function attachmentUrl(a: VizierAttachment): string | null {
    const c = a.content as {
        local?: string
        url?: string
        artifact?: { agent_id: string; artifact_id: string; available?: boolean }
    }
    if (c.artifact) {
        if (c.artifact.available === false) return null
        return artifactContentUrl(c.artifact.agent_id, c.artifact.artifact_id)
    }
    // An upload's `local` is a path on the engine, which only the transport knows how to address;
    // a `url` is somebody else's, and is loaded as it is.
    if (c.local) return assetPath(c.local)
    return c.url ?? null
}

/**
 * Where the attachment's bytes are under the API, for the transport to fetch. Null for one that isn't
 * ours to fetch: a remote URL, or an artifact that has gone.
 */
export function attachmentPath(a: VizierAttachment): string | null {
    const c = a.content as { local?: string; artifact?: { agent_id: string; artifact_id: string; available?: boolean } }
    if (c.artifact) {
        if (c.artifact.available === false) return null
        return `/agents/${c.artifact.agent_id}/artifacts/${encodeURIComponent(c.artifact.artifact_id)}/content`
    }
    return c.local ? relativeToApi(c.local) : null
}

function asPreview(a: VizierAttachment, path: string): PreviewFile {
    const ext = EXT(a.filename)
    return {
        key: path,
        name: a.filename,
        mimeType: IMAGE.has(ext) ? `image/${ext}` : (MIME[ext] ?? 'application/octet-stream'),
        size: 0,
        url: path,
    }
}

const openElsewhere = (src: string) => window.open(src, '_blank', 'noopener')

/** A file downloads through the transport when it is ours; a remote one opens where it lives. */
function download(a: VizierAttachment, src: string) {
    const path = attachmentPath(a)
    if (path) return void downloadFile(path, a.filename)
    openElsewhere(src)
}

export function Audio({ src, transcript }: { src: string; transcript?: string }) {
    const el = useRef<HTMLAudioElement>(null)
    const [playing, setPlaying] = useState(false)
    const [position, setPosition] = useState(0)
    const [duration, setDuration] = useState(0)
    const [speed, setSpeed] = useState(1)
    return (
        <>
            <audio
                ref={el}
                src={src}
                preload="metadata"
                onLoadedMetadata={(e) => setDuration(Number.isFinite(e.currentTarget.duration) ? e.currentTarget.duration : 0)}
                onTimeUpdate={(e) => setPosition(e.currentTarget.currentTime)}
                onEnded={() => setPlaying(false)}
            />
            <AudioPlayer
                playing={playing}
                onPlayingChange={(p) => {
                    setPlaying(p)
                    if (p) void el.current?.play()
                    else el.current?.pause()
                }}
                position={position}
                duration={duration}
                speed={speed}
                onSpeedChange={(s) => {
                    setSpeed(s)
                    if (el.current) el.current.playbackRate = s
                }}
                transcript={transcript}
            />
        </>
    )
}

export function Video({ a, src }: { a: Pick<VizierAttachment, 'filename'>; src: string }) {
    const [on, setOn] = useState(false)
    return on ? (
        <video src={src} controls autoPlay className="max-h-360 w-full rounded-lg" />
    ) : (
        <VideoPlayer title={`Play ${a.filename}`} caption={a.filename} onPlay={() => setOn(true)} />
    )
}

/** What a reply carries: images side by side, audio and video players, other files as peeks. */
export function ReplyMedia({ attachments, transcript }: { attachments: VizierAttachment[]; transcript?: string | null }) {
    const [open, setOpen] = useState<number | null>(null)
    const items = attachments
        .map((a) => ({ a, url: attachmentUrl(a) }))
        .filter((x): x is { a: VizierAttachment; url: string } => x.url !== null)
    const unavailable = attachments.filter((a) => {
        const c = a.content as { artifact?: { available?: boolean } }
        return c.artifact?.available === false
    })
    if (items.length === 0 && unavailable.length === 0) return null

    const images = items.filter((x) => IMAGE.has(EXT(x.a.filename)))
    const others = items.filter((x) => !IMAGE.has(EXT(x.a.filename)))
    // The preview reads through the transport, so only files of ours can open in it.
    const previews = items.flatMap((x) => {
        const path = attachmentPath(x.a)
        return path ? [{ url: x.url, file: asPreview(x.a, path) }] : []
    })
    const openFile = (url: string) => {
        const index = previews.findIndex((p) => p.url === url)
        if (index >= 0) setOpen(index)
        else openElsewhere(url)
    }

    return (
        <div className="flex flex-col gap-8">
            {unavailable.map((a) => (
                <FilePeek
                    key={`unavailable-${a.filename}`}
                    name={a.filename}
                    meta="artifact unavailable"
                    failed
                    action={{ label: 'Retry', onClick: () => window.location.reload() }}
                />
            ))}
            {images.length > 0 && (
                <ImageGallery
                    images={images.map((x) => ({ src: x.url, name: x.a.filename }))}
                    onOpen={(i) => openFile(images[i].url)}
                />
            )}
            {others.map(({ a, url }) => {
                const ext = EXT(a.filename)
                if (AUDIO.has(ext)) return <Audio key={url} src={url} transcript={transcript ?? undefined} />
                if (VIDEO.has(ext)) return <Video key={url} a={a} src={url} />
                const previewable = ext in MIME && ext !== 'pdf'
                return (
                    <FilePeek
                        key={url}
                        name={a.filename}
                        meta={previewable ? undefined : 'no preview for this type'}
                        action={
                            previewable
                                ? { label: 'Open', onClick: () => openFile(url) }
                                : { label: 'Download', onClick: () => download(a, url) }
                        }
                    />
                )
            })}
            {open !== null && <FilePreviewModal files={previews.map((p) => p.file)} index={open} onIndex={setOpen} onClose={() => setOpen(null)} />}
        </div>
    )
}
