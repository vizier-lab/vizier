import {
    Attachment,
    Composer as LibraryComposer,
    DropZone,
    FilterTabs,
    Menu,
    MenuItem,
    Switch,
    VoiceRecorder,
    type SkillMenuItem,
} from '@vizier/design'
import { useCallback, useEffect, useRef, useState } from 'react'
import { api } from '../../api/client'
import {
    suggestProjectFiles,
    type FileSuggestion,
    type ProjectMode,
} from '../../api/endpoints/projects'
import { insertMention, mentionQuery, pickedMentions } from '../../lib/mentions'
import { abbreviate } from '../../lib/format'
import { useMobile } from '../../lib/useMobile'
import { toast } from '../../stores/toasts'
import { fileSize, MAX_UPLOAD_BYTES, uploadFile } from '../../lib/upload'
import { WavRecorder } from '../../lib/wavRecorder'
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
    { name: 'btw', description: 'Ask a side question that is never saved' },
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
        spoken: boolean,
        mentions?: string[]
    ) => void
    onSendAudio: (attachment: VizierAttachment) => void
    onCommand: (command: 'abort' | 'checkpoint' | 'lobotomy') => void
    onStop: () => void
    /** `/btw <question>`: opens the side chat window, which asks `question` at once. */
    onSideChat?: (question: string) => void
    /** The project chat this composer is in (spec 041): its saved mode and id. */
    project?: { id: string; mode: ProjectMode }
    onModeChange?: (mode: ProjectMode) => void
    topicId?: string
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
    onSideChat,
    project,
    onModeChange,
    topicId,
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
    // Plan/Build belongs to the chat, not to the composer's text; a switch applies next turn.
    const [mode, setMode] = useState<ProjectMode>(project?.mode ?? 'plan')
    useEffect(() => setMode(project?.mode ?? 'plan'), [project?.mode])
    const [modeNote, setModeNote] = useState(false)
    const [picked, setPicked] = useState<string[]>([])
    const [suggestions, setSuggestions] = useState<{ at: number; items: FileSuggestion[] } | null>(null)
    const suggestionRequest = useRef(0)

    // `@` suggestions, debounced 120 ms. A stale response is dropped by request id.
    useEffect(() => {
        const token = mentionQuery(value)
        if (!project || !token) {
            setSuggestions(null)
            return
        }
        const id = ++suggestionRequest.current
        const timer = setTimeout(() => {
            suggestProjectFiles(project.id, agentId, token.query, topicId)
                .then((items) => {
                    if (suggestionRequest.current === id)
                        setSuggestions({ at: token.start, items: items ?? [] })
                })
                .catch(() => {
                    if (suggestionRequest.current === id) setSuggestions(null)
                })
        }, 120)
        return () => clearTimeout(timer)
    }, [value, project, agentId, topicId])

    const pick = (path: string) => {
        const token = mentionQuery(value)
        if (!token) return
        setValue(insertMention(value, token.start, path))
        setPicked((all) => (all.includes(path) ? all : [...all, path]))
        setSuggestions(null)
        input.current?.focus()
    }
    // A spoken reply needs the agent's Text-to-Speech; without it there is no switch to keep on.
    useEffect(() => {
        if (!ttsEnabled) setSpoken(false)
    }, [ttsEnabled])
    const input = useRef<HTMLInputElement>(null)
    const recorder = useRef<WavRecorder | null>(null)
    const timer = useRef<ReturnType<typeof setInterval> | null>(null)
    const [playing, setPlaying] = useState(false)
    const [progress, setProgress] = useState(0)
    const player = useRef<HTMLAudioElement | null>(null)

    // The recording, ready to listen back to before it is sent.
    const recordedBlob = voice?.blob
    useEffect(() => {
        setPlaying(false)
        setProgress(0)
        if (!recordedBlob) return
        const url = URL.createObjectURL(recordedBlob)
        const el = new Audio(url)
        player.current = el
        el.ontimeupdate = () =>
            setProgress(el.duration > 0 ? el.currentTime / el.duration : 0)
        el.onended = () => {
            setPlaying(false)
            setProgress(0)
        }
        return () => {
            el.pause()
            player.current = null
            URL.revokeObjectURL(url)
        }
    }, [recordedBlob])

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
        // Before the command regex and before anything is uploaded or sent: a side chat is never saved.
        const btw = /^\/btw(?:\s+([\s\S]*))?$/.exec(text)
        if (btw) {
            // A throwaway question has no box of its own to type in: without one, leave `/btw`
            // in the composer so the question can be finished here.
            const question = btw[1]?.trim()
            if (!question) return
            onSideChat?.(question)
            setValue('')
            return
        }
        // Enter in the slash menu completes the name and calls this with what was typed so far:
        // `/b` is the menu's to complete (a second Enter then runs it), never a message.
        const partial = /^\/(\S*)$/.exec(text)?.[1]?.toLowerCase()
        if (
            partial !== undefined &&
            !skills.some((s) => s.name.toLowerCase() === partial) &&
            skills.some((s) => s.name.toLowerCase().includes(partial))
        )
            return
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
            onSend(text, uploaded, spoken, pickedMentions(text, picked))
            setValue('')
            setPicked([])
            setSuggestions(null)
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
        if (!navigator.mediaDevices?.getUserMedia || typeof MediaRecorder === 'undefined') {
            toast.error("Audio recording isn't available in this window.")
            return
        }
        try {
            const stream = await navigator.mediaDevices.getUserMedia({
                audio: true,
            })
            let rec: WavRecorder
            try {
                rec = await WavRecorder.start(stream)
            } catch (e) {
                stream.getTracks().forEach((t) => t.stop())
                throw e
            }
            recorder.current = rec
            rec.onstop = (blob) => {
                stopTimer()
                setVoice((v) => (v ? { ...v, mode: 'recorded', blob } : v))
            }
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

    useEffect(
        () => () => {
            stopTimer()
            recorder.current?.cancel()
        },
        []
    )

    /** Drops the recording, whether it is still being made or already listened back to. */
    const discardVoice = () => {
        recorder.current?.cancel()
        stopTimer()
        setVoice(null)
    }

    const sendVoice = async () => {
        // Sending mid-recording stops it first and sends what was recorded.
        const blob =
            voice?.mode === 'recording' ? recorder.current?.stop() : voice?.blob
        if (!blob) return
        player.current?.pause()
        try {
            onSendAudio(
                await uploadFile(
                    new File([blob], 'voice-message.wav', { type: blob.type })
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
            {suggestions && suggestions.items.length > 0 && (
                <div className="absolute inset-x-0 bottom-full z-20 mb-8 px-24">
                    <Menu aria-label="Mention a file" className="max-h-280 overflow-y-auto">
                        {suggestions.items.map((item) => (
                            <MenuItem
                                key={item.path}
                                icon={item.kind === 'dir' ? 'folder' : 'file'}
                                label={item.path}
                                onClick={() => pick(item.path)}
                            />
                        ))}
                    </Menu>
                </div>
            )}
            <LibraryComposer
                agentName={agentName}
                value={value}
                onValueChange={setValue}
                onSend={() =>
                    void (voice ? sendVoice() : send())
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
                            onDiscard={discardVoice}
                            playing={playing}
                            progress={progress}
                            onPlayingChange={(p) => {
                                setPlaying(p)
                                if (p) void player.current?.play()
                                else player.current?.pause()
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
                    (project || ttsEnabled) && (
                        <div className="flex flex-wrap items-center gap-12">
                            {project && (
                                <div className="flex items-center gap-8">
                                    <FilterTabs<ProjectMode>
                                        aria-label="Plan or Build"
                                        value={mode}
                                        onChange={(next) => {
                                            if (next === mode) return
                                            setMode(next)
                                            setModeNote(busy)
                                            onModeChange?.(next)
                                        }}
                                        options={[
                                            { value: 'plan', label: 'Plan' },
                                            { value: 'build', label: 'Build' },
                                        ]}
                                    />
                                    <span className="type-small text-faint">
                                        {mode === 'plan'
                                            ? "Read-only: can look, can't change anything"
                                            : 'Can edit files and run commands'}
                                        {modeNote ? ' · applies to the next message' : ''}
                                    </span>
                                </div>
                            )}
                            {ttsEnabled && (
                                <label className="flex items-center gap-8 type-small text-muted">
                                    <Switch
                                        checked={spoken}
                                        onCheckedChange={setSpoken}
                                        aria-label="Ask for a spoken reply"
                                    />
                                    Ask for a spoken reply
                                </label>
                            )}
                        </div>
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
