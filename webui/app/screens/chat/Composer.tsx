import {
    Attachment,
    Composer as LibraryComposer,
    DropZone,
    Switch,
    VoiceRecorder,
    type SkillMenuItem,
} from '@vizier/design'
import { useCallback, useEffect, useRef, useState } from 'react'
import { api } from '../../api/client'
import { abbreviate } from '../../lib/format'
import { useMobile } from '../../lib/useMobile'
import { toast } from '../../stores/toasts'
import { fileSize, MAX_UPLOAD_BYTES, uploadFile } from '../../lib/upload'
import type { VizierAttachment } from '../../api/frames'

const BUILTIN: SkillMenuItem[] = [
    {
        name: 'checkpoint',
        description: 'Hand the conversation over to a fresh context',
    },
    {
        name: 'lobotomy',
        description: 'Clear the conversation context without a handover',
    },
    { name: 'abort', description: 'Stop what the agent is doing' },
    { name: 'skills', description: "List this agent's skills" },
]

type Pending = {
    id: number
    file: File
    state: 'ready' | 'failed'
    reason?: string
}

type Props = {
    agentId: string
    agentName: string
    /** The agent has Speech-to-Text, so the mic is offered. */
    sttEnabled: boolean
    /** The agent has Text-to-Speech, so "Ask for a spoken reply" is offered. */
    ttsEnabled: boolean
    busy: boolean
    /** Why it can't send (the agent is down), in place of the placeholder. */
    disabledReason?: string
    context: { used: number; window: number; detail?: string } | null
    onSend: (
        text: string,
        attachments: VizierAttachment[],
        spoken: boolean
    ) => void
    onSendAudio: (attachment: VizierAttachment) => void
    onCommand: (command: 'abort' | 'checkpoint' | 'lobotomy') => void
    onStop: () => void
}

let pendingId = 1

/** The library Composer wired to uploads, voice, the slash menu and the context ring. */
export function Composer({
    agentId,
    agentName,
    sttEnabled,
    ttsEnabled,
    busy,
    disabledReason,
    context,
    onSend,
    onSendAudio,
    onCommand,
    onStop,
}: Props) {
    const mobile = useMobile()
    const [value, setValue] = useState('')
    const [pending, setPending] = useState<Pending[]>([])
    const [skills, setSkills] = useState<SkillMenuItem[]>(BUILTIN)
    const [dragging, setDragging] = useState(false)
    const [voice, setVoice] = useState<{
        mode: 'recording' | 'recorded'
        elapsed: number
        blob?: Blob
    } | null>(null)
    const [spoken, setSpoken] = useState(false)
    // A spoken reply needs the agent's Text-to-Speech; without it there is no switch to keep on.
    useEffect(() => {
        if (!ttsEnabled) setSpoken(false)
    }, [ttsEnabled])
    const input = useRef<HTMLInputElement>(null)
    const recorder = useRef<MediaRecorder | null>(null)
    const timer = useRef<ReturnType<typeof setInterval> | null>(null)

    useEffect(() => {
        let alive = true
        Promise.allSettled([
            api.get<{ name: string; description: string }[]>(
                `/agents/${agentId}/skills`
            ),
            api.get<{ name: string; description: string }[]>('/skills'),
        ]).then(([mine, global]) => {
            if (!alive) return
            const list = [
                ...(mine.status === 'fulfilled' ? (mine.value ?? []) : []),
                ...(global.status === 'fulfilled' ? (global.value ?? []) : []),
            ]
            const seen = new Set<string>()
            const skills = list
                .filter((s) =>
                    seen.has(s.name) ? false : (seen.add(s.name), true)
                )
                .map((s) => ({ name: s.name, description: s.description }))
            setSkills([...BUILTIN, ...skills])
        })
        return () => {
            alive = false
        }
    }, [agentId])

    const add = useCallback((files: File[]) => {
        setPending((p) => [
            ...p,
            ...files.map((file): Pending =>
                file.size > MAX_UPLOAD_BYTES
                    ? {
                          id: pendingId++,
                          file,
                          state: 'failed',
                          reason: 'over 25 MB',
                      }
                    : { id: pendingId++, file, state: 'ready' }
            ),
        ])
    }, [])

    const send = async () => {
        const text = value.trim()
        const command = /^\/(checkpoint|lobotomy|abort)$/.exec(text)?.[1] as
            'abort' | 'checkpoint' | 'lobotomy' | undefined
        if (command) {
            onCommand(command)
            setValue('')
            return
        }
        const ready = pending.filter((p) => p.state === 'ready')
        if (!text && ready.length === 0) return
        try {
            const uploaded = await Promise.all(
                ready.map((p) => uploadFile(p.file))
            )
            onSend(text, uploaded, spoken)
            setValue('')
            setPending([])
        } catch (e) {
            toast.error(
                "Couldn't upload the attachment",
                e instanceof Error ? e.message : undefined
            )
        }
    }

    const stopTimer = () => {
        if (timer.current) clearInterval(timer.current)
        timer.current = null
    }

    const startRecording = async () => {
        try {
            const stream = await navigator.mediaDevices.getUserMedia({
                audio: true,
            })
            const chunks: Blob[] = []
            const rec = new MediaRecorder(stream)
            recorder.current = rec
            rec.ondataavailable = (e) => chunks.push(e.data)
            rec.onstop = () => {
                stream.getTracks().forEach((t) => t.stop())
                stopTimer()
                setVoice((v) =>
                    v
                        ? {
                              ...v,
                              mode: 'recorded',
                              blob: new Blob(chunks, { type: rec.mimeType }),
                          }
                        : v
                )
            }
            rec.start()
            setVoice({ mode: 'recording', elapsed: 0 })
            timer.current = setInterval(
                () =>
                    setVoice((v) => (v ? { ...v, elapsed: v.elapsed + 1 } : v)),
                1000
            )
        } catch {
            toast.error(
                'Microphone unavailable',
                'Allow microphone access to record a voice message.'
            )
        }
    }

    useEffect(() => () => stopTimer(), [])

    const sendVoice = async () => {
        if (!voice?.blob) return
        const ext = voice.blob.type.includes('ogg')
            ? 'ogg'
            : voice.blob.type.includes('mp4')
              ? 'm4a'
              : 'webm'
        try {
            onSendAudio(
                await uploadFile(
                    new File([voice.blob], `voice-message.${ext}`, {
                        type: voice.blob.type,
                    })
                )
            )
            setVoice(null)
        } catch (e) {
            toast.error(
                "Couldn't send the voice message",
                e instanceof Error ? e.message : undefined
            )
        }
    }

    return (
        <div
            className="relative"
            onDragEnter={(e) => {
                if (e.dataTransfer.types.includes('Files')) setDragging(true)
            }}
            onDragLeave={(e) => {
                if (!e.currentTarget.contains(e.relatedTarget as Node))
                    setDragging(false)
            }}
            onDragOver={(e) => e.preventDefault()}
            onDrop={() => setDragging(false)}
            onPaste={(e) => {
                const files = Array.from(e.clipboardData.files)
                if (files.length) add(files)
            }}
        >
            <LibraryComposer
                agentName={agentName}
                value={value}
                onValueChange={setValue}
                onSend={() =>
                    void (voice?.mode === 'recorded' ? sendVoice() : send())
                }
                busy={busy}
                onStop={onStop}
                disabledReason={disabledReason}
                skills={skills}
                context={context ?? undefined}
                tokenText={
                    mobile && context
                        ? `${abbreviate(context.used)} / ${abbreviate(context.window)}`
                        : undefined
                }
                mic={sttEnabled}
                recording={voice?.mode === 'recording'}
                onAttach={() => input.current?.click()}
                onMic={() =>
                    voice
                        ? recorder.current?.state === 'recording' &&
                          recorder.current.stop()
                        : void startRecording()
                }
                voice={
                    voice && (
                        <VoiceRecorder
                            mode={voice.mode}
                            elapsed={voice.elapsed}
                            onStop={() => recorder.current?.stop()}
                            onDiscard={() => {
                                stopTimer()
                                setVoice(null)
                            }}
                        />
                    )
                }
                overlay={
                    <DropZone
                        look="overlay"
                        active={dragging}
                        hint="Files, images and audio"
                        onFiles={(files) => {
                            setDragging(false)
                            add(files)
                        }}
                    />
                }
                below={
                    ttsEnabled && (
                        <label className="flex items-center gap-8 type-small text-muted">
                            <Switch
                                checked={spoken}
                                onCheckedChange={setSpoken}
                                aria-label="Ask for a spoken reply"
                            />
                            Ask for a spoken reply
                        </label>
                    )
                }
                attachments={
                    pending.length > 0 ? (
                        <>
                            {pending.map((p) => (
                                <Attachment
                                    key={p.id}
                                    name={p.file.name}
                                    state={p.state}
                                    detail={p.reason ?? fileSize(p.file.size)}
                                    onRemove={() =>
                                        setPending((all) =>
                                            all.filter((x) => x.id !== p.id)
                                        )
                                    }
                                />
                            ))}
                        </>
                    ) : undefined
                }
            />
            <input
                ref={input}
                type="file"
                multiple
                hidden
                onChange={(e) => {
                    add(Array.from(e.target.files ?? []))
                    e.target.value = ''
                }}
            />
        </div>
    )
}
