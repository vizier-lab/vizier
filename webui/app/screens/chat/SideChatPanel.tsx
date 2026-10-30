import { AgentReply, Button } from '@vizier/design'
import { useEffect, useReducer, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react'
import { createPortal } from 'react-dom'
import { ApiError } from '../../api/client'
import { askSideChat } from '../../api/endpoints/chat'
import { clampRect, defaultRect, moved, resized, type Rect } from '../../lib/floatingWindow'
import { initialSideChat, mapAskError, refusal, sideChatReducer } from '../../lib/sideChat'
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
    /** The question typed after `/btw`, asked as soon as the window opens. */
    question?: string
}

/**
 * `/btw`: a small floating window that shows the agent's answer to one throwaway question,
 * asked beside the open conversation. It has no input of its own: another question is another
 * `/btw`. It has no scrim, so the conversation stays visible and usable behind it; the header
 * moves it and the bottom-right corner resizes it. The answer lives in component state only;
 * closing, switching conversation or unmounting discards it, and an ask still in flight is
 * aborted so a late answer never shows in a reopened window.
 */
export function SideChatPanel({
    open,
    onClose,
    agentId,
    agentName,
    modelName,
    channelId,
    topicId,
    question,
}: Props) {
    const [state, dispatch] = useReducer(sideChatReducer, initialSideChat)
    const controller = useRef<AbortController | null>(null)
    const frame = useRef<HTMLDivElement>(null)
    const [rect, setRect] = useState<Rect>(() => {
        const { vw, vh } = viewport()
        return clampRect(remembered ?? defaultRect(vw, vh), vw, vh)
    })
    const drag = useRef<{ kind: 'move' | 'resize'; x: number; y: number; start: Rect } | null>(null)

    const turn = state.turns[0]
    // A question the server would refuse is said so, without a request.
    const problem = question?.trim() ? refusal(question) : null

    const run = (text: string) => {
        controller.current?.abort()
        const own = new AbortController()
        controller.current = own
        askSideChat(
            agentId,
            { channel_id: channelId, topic_id: topicId, question: text, turns: [] },
            own.signal
        )
            .then((res) => {
                if (!own.signal.aborted) dispatch({ type: 'answered', index: 0, answer: res.answer })
            })
            .catch((e: unknown) => {
                if (own.signal.aborted) return
                const err = e instanceof ApiError ? e : undefined
                dispatch({ type: 'failed', index: 0, ...mapAskError(err?.status, err?.message ?? '') })
            })
    }

    // The window belongs to one open `/btw` on one conversation: leaving either discards it.
    useEffect(() => {
        if (!open || !question?.trim() || problem) return
        dispatch({ type: 'ask', question })
        run(question.trim())
        return () => {
            controller.current?.abort()
            controller.current = null
            dispatch({ type: 'reset' })
        }
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [open, agentId, topicId, question])

    // Esc closes it when focus is in the window, so the window takes focus as it opens.
    useEffect(() => {
        if (open) frame.current?.focus()
    }, [open])

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

    if (!open) return null

    return createPortal(
        <div
            ref={frame}
            role="dialog"
            aria-label="Side chat"
            tabIndex={-1}
            onKeyDown={(e) => {
                if (e.key === 'Escape') {
                    e.stopPropagation()
                    onClose()
                }
            }}
            style={{ left: rect.x, top: rect.y, width: rect.w, height: rect.h }}
            className="fixed z-50 flex flex-col overflow-hidden rounded-xl border border-border-strong bg-surface shadow-dialog outline-none"
        >
            <header
                onPointerDown={begin('move')}
                onPointerMove={track}
                onPointerUp={end}
                onPointerCancel={end}
                className="flex shrink-0 cursor-grab touch-none select-none items-center gap-12 border-b border-border px-20 py-12 active:cursor-grabbing"
            >
                <span className="type-code font-medium text-live-text">/btw</span>
                <span className="min-w-0 flex-1 truncate text-muted">{question}</span>
                <span
                    onPointerDown={(e) => e.stopPropagation()}
                    onClick={onClose}
                    className="type-code cursor-pointer text-muted hover:text-fg"
                >
                    esc
                </span>
            </header>

            <div className="flex min-h-0 flex-1 flex-col gap-16 overflow-y-auto px-20 py-16">
                <p className="text-sm text-muted">
                    {agentName}
                    {modelName ? ` · ${modelName}` : ''}. Not saved — closing this discards it.
                </p>
                {question?.trim() && <p className="font-medium text-fg">{question.trim()}</p>}
                {problem ? (
                    <p className="text-sm text-fg">{problem}</p>
                ) : turn?.answer != null ? (
                    <AgentReply>
                        <Markdown>{turn.answer}</Markdown>
                    </AgentReply>
                ) : turn?.error ? (
                    <AgentReply
                        state="failed"
                        error={
                            <span className="flex items-center gap-8">
                                {turn.error.message}
                                {turn.error.retryable && (
                                    <Button
                                        size="sm"
                                        variant="secondary"
                                        onClick={() => {
                                            dispatch({ type: 'retry', index: 0 })
                                            run(turn.question)
                                        }}
                                    >
                                        Retry
                                    </Button>
                                )}
                            </span>
                        }
                    >
                        <span />
                    </AgentReply>
                ) : (
                    <p className="text-sm text-muted" aria-busy="true">
                        Thinking…
                    </p>
                )}
            </div>

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
