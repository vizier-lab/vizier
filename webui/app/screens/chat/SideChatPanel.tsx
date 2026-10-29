import { AgentReply, Button, Textarea } from '@vizier/design'
import { useEffect, useReducer, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react'
import { createPortal } from 'react-dom'
import { ApiError } from '../../api/client'
import { askSideChat } from '../../api/endpoints/chat'
import {
    charsLeft,
    initialSideChat,
    isFull,
    mapAskError,
    refusal,
    sideChatReducer,
    toTurns,
    type SideChatTurn,
} from '../../lib/sideChat'
import { clampRect, defaultRect, moved, resized, type Rect } from '../../lib/floatingWindow'
import { Markdown } from '../shared/Markdown'

/** Where the window was last left, for the next `/btw` in this page. Memory only: never stored. */
let remembered: Rect | null = null

const viewport = () => ({ vw: window.innerWidth, vh: window.innerHeight })

type Props = {
    open: boolean
    onClose: () => void
    agentId: string
    agentName: string
    /** The model the agent answers with, shown read-only. */
    modelName?: string
    channelId: string
    topicId: string
    /** Asked at once when the panel opens. */
    initialQuestion?: string
}

/**
 * `/btw`: a small floating window that asks the agent beside the open conversation. It has no
 * scrim, so the conversation stays visible and usable behind it; the header moves it and the
 * bottom-right corner resizes it. Its exchange lives in component state only; closing,
 * switching conversation or unmounting discards it, and any ask still in flight is aborted so
 * a late answer never shows in a reopened window.
 */
export function SideChatPanel({
    open,
    onClose,
    agentId,
    agentName,
    modelName,
    channelId,
    topicId,
    initialQuestion,
}: Props) {
    const [state, dispatch] = useReducer(sideChatReducer, initialSideChat)
    const [draft, setDraft] = useState('')
    const controllers = useRef(new Map<number, AbortController>())
    const body = useRef<HTMLDivElement>(null)
    const [rect, setRect] = useState<Rect>(() => {
        const { vw, vh } = viewport()
        return clampRect(remembered ?? defaultRect(vw, vh), vw, vh)
    })
    const drag = useRef<{ kind: 'move' | 'resize'; x: number; y: number; start: Rect } | null>(null)

    const abortAll = () => {
        controllers.current.forEach((c) => c.abort())
        controllers.current.clear()
    }

    // Each run belongs to one open modal on one conversation: leaving either discards it.
    useEffect(() => {
        if (!open) return
        if (initialQuestion?.trim()) ask(initialQuestion)
        return () => {
            abortAll()
            dispatch({ type: 'reset' })
            setDraft('')
        }
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [open, agentId, topicId])

    // A smaller browser window pulls this one back in.
    useEffect(() => {
        const fit = () => setRect((r) => clampRect(r, window.innerWidth, window.innerHeight))
        window.addEventListener('resize', fit)
        return () => window.removeEventListener('resize', fit)
    }, [])

    // Pointer capture keeps a drag going when the pointer outruns the window.
    const begin = (kind: 'move' | 'resize') => (e: ReactPointerEvent<HTMLElement>) => {
        if (e.button !== 0) return
        e.preventDefault()
        e.currentTarget.setPointerCapture(e.pointerId)
        drag.current = { kind, x: e.clientX, y: e.clientY, start: rect }
    }
    const track = (e: ReactPointerEvent<HTMLElement>) => {
        const d = drag.current
        if (!d) return
        const { vw, vh } = viewport()
        const change = d.kind === 'move' ? moved : resized
        setRect(change(d.start, e.clientX - d.x, e.clientY - d.y, vw, vh))
    }
    const end = (e: ReactPointerEvent<HTMLElement>) => {
        if (!drag.current) return
        drag.current = null
        e.currentTarget.releasePointerCapture(e.pointerId)
        setRect((r) => (remembered = r))
    }

    // Keep the newest turn in view as answers arrive.
    useEffect(() => {
        const el = body.current
        if (el) el.scrollTop = el.scrollHeight
    }, [state.turns, state.pending])

    const run = (index: number, question: string, earlier: SideChatTurn[]) => {
        const controller = new AbortController()
        controllers.current.set(index, controller)
        askSideChat(
            agentId,
            { channel_id: channelId, topic_id: topicId, question, turns: toTurns(earlier) },
            controller.signal
        )
            .then((res) => {
                if (controller.signal.aborted) return
                dispatch({ type: 'answered', index, answer: res.answer })
            })
            .catch((e: unknown) => {
                if (controller.signal.aborted) return
                const err = e instanceof ApiError ? e : undefined
                dispatch({
                    type: 'failed',
                    index,
                    ...mapAskError(err?.status, err?.message ?? ''),
                })
            })
            .finally(() => controllers.current.delete(index))
    }

    const ask = (question: string) => {
        if (refusal(state.turns, question)) return
        const index = state.turns.length
        dispatch({ type: 'ask', question })
        run(index, question.trim(), state.turns)
    }

    const retry = (index: number) => {
        const turn = state.turns[index]
        if (!turn) return
        dispatch({ type: 'retry', index })
        run(index, turn.question, state.turns.slice(0, index))
    }

    const refuse = draft.trim() ? refusal(state.turns, draft) : null
    const full = isFull(state.turns)
    const busy = state.pending != null

    const submit = () => {
        if (busy || refuse) return
        ask(draft)
        setDraft('')
    }

    if (!open) return null

    return createPortal(
        <div
            role="dialog"
            aria-label="Side chat"
            onKeyDown={(e) => {
                if (e.key === 'Escape') {
                    e.stopPropagation()
                    onClose()
                }
            }}
            style={{ left: rect.x, top: rect.y, width: rect.w, height: rect.h }}
            className="fixed z-50 flex flex-col overflow-hidden rounded-xl border border-border-strong bg-surface shadow-dialog"
        >
            <header
                onPointerDown={begin('move')}
                onPointerMove={track}
                onPointerUp={end}
                onPointerCancel={end}
                className="flex shrink-0 cursor-grab touch-none select-none items-center gap-12 border-b border-border px-20 py-12 active:cursor-grabbing"
            >
                <span className="type-code font-medium text-live-text">/btw</span>
                <span className="min-w-0 flex-1 truncate text-muted">
                    {state.turns[0]?.question ?? `Ask ${agentName}`}
                </span>
                <span
                    onPointerDown={(e) => e.stopPropagation()}
                    onClick={onClose}
                    className="type-code cursor-pointer text-muted hover:text-fg"
                >
                    esc
                </span>
            </header>

            <div ref={body} className="flex min-h-0 flex-1 flex-col gap-16 overflow-y-auto px-20 py-16">
                <p className="text-sm text-muted">
                    {agentName}
                    {modelName ? ` · ${modelName}` : ''}. Not saved — closing this discards it.
                </p>
                {state.turns.map((turn, i) => (
                    <SideChatEntry
                        key={i}
                        turn={turn}
                        pending={state.pending === i}
                        onRetry={() => retry(i)}
                    />
                ))}
            </div>

            <footer className="flex shrink-0 flex-col gap-8 border-t border-border px-20 py-12">
                <Textarea
                    aria-label="Side chat question"
                    rows={2}
                    autoFocus
                    value={draft}
                    disabled={full}
                    placeholder={
                        full
                            ? 'This side chat is full — start a new one'
                            : `Ask ${agentName} something off the record`
                    }
                    onChange={(e) => setDraft(e.target.value)}
                    onKeyDown={(e) => {
                        if (e.key === 'Enter' && !e.shiftKey) {
                            e.preventDefault()
                            submit()
                        }
                    }}
                />
                <div className="flex items-center justify-between gap-8">
                    <span className="text-sm text-muted">
                        {refuse ?? `${charsLeft(state.turns).toLocaleString()} characters left · enter to ask`}
                    </span>
                    <Button size="sm" onClick={submit} disabled={busy || full || !!refuse}>
                        Ask
                    </Button>
                </div>
            </footer>

            <div
                aria-hidden
                title="Drag to resize"
                onPointerDown={begin('resize')}
                onPointerMove={track}
                onPointerUp={end}
                onPointerCancel={end}
                className="absolute right-0 bottom-0 size-20 cursor-nwse-resize touch-none"
            >
                <svg viewBox="0 0 20 20" className="size-full text-muted">
                    <path d="M18 8 8 18M18 13 13 18" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
                </svg>
            </div>
        </div>,
        document.body
    )
}

function SideChatEntry({
    turn,
    pending,
    onRetry,
}: {
    turn: SideChatTurn
    pending: boolean
    onRetry: () => void
}) {
    return (
        <div className="flex flex-col gap-8">
            <p className="font-medium text-fg">{turn.question}</p>
            {turn.answer != null ? (
                <AgentReply>
                    <Markdown>{turn.answer}</Markdown>
                </AgentReply>
            ) : turn.error ? (
                <AgentReply
                    state="failed"
                    error={
                        <span className="flex items-center gap-8">
                            {turn.error.message}
                            {turn.error.retryable && (
                                <Button size="sm" variant="secondary" onClick={onRetry}>
                                    Retry
                                </Button>
                            )}
                        </span>
                    }
                >
                    <span />
                </AgentReply>
            ) : pending ? (
                <p className="text-sm text-muted" aria-busy="true">
                    Thinking…
                </p>
            ) : null}
        </div>
    )
}
