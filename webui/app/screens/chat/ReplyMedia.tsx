import { AudioPlayer, FilePeek, ImageGallery, VideoPlayer } from '@vizier/design'
import { useRef, useState } from 'react'
import type { SessionFileRecord, VizierAttachment } from '../../api/frames'
import { FilePreviewDialog } from './FilePreviewDialog'

const EXT = (name: string) => name.split('.').pop()?.toLowerCase() ?? ''
const IMAGE = new Set(['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'avif'])
const AUDIO = new Set(['mp3', 'wav', 'ogg', 'oga', 'm4a', 'webm', 'opus', 'flac'])
const VIDEO = new Set(['mp4', 'mov', 'mkv', 'm4v'])
const MIME: Record<string, string> = { csv: 'text/csv', md: 'text/markdown', txt: 'text/plain', json: 'application/json', pdf: 'application/pdf' }

export function attachmentUrl(a: VizierAttachment): string | null {
    const c = a.content as Record<string, string>
    return c.local ?? c.url ?? null
}

/** A synthetic record, so an attachment opens in the same preview as a session file. */
function asRecord(a: VizierAttachment, url: string): SessionFileRecord {
    const ext = EXT(a.filename)
    return {
        id: url,
        session_slug: '',
        agent_id: '',
        filename: a.filename,
        mime_type: IMAGE.has(ext) ? `image/${ext}` : (MIME[ext] ?? 'application/octet-stream'),
        size: 0,
        file_id: url.split('/').pop() ?? '',
        added_at: '',
    }
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
    if (items.length === 0) return null

    const images = items.filter((x) => IMAGE.has(EXT(x.a.filename)))
    const others = items.filter((x) => !IMAGE.has(EXT(x.a.filename)))
    const records = items.map((x) => asRecord(x.a, x.url))

    return (
        <div className="flex flex-col gap-8">
            {images.length > 0 && (
                <ImageGallery
                    images={images.map((x) => ({ src: x.url, name: x.a.filename }))}
                    onOpen={(i) => setOpen(items.indexOf(images[i]))}
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
                                ? { label: 'Open', onClick: () => setOpen(items.indexOf(items.find((x) => x.url === url)!)) }
                                : { label: 'Download', onClick: () => window.open(url, '_blank', 'noopener') }
                        }
                    />
                )
            })}
            {open !== null && <FilePreviewDialog files={records} index={open} onIndex={setOpen} onClose={() => setOpen(null)} />}
        </div>
    )
}
