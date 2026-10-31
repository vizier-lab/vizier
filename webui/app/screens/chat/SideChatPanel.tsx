import { AgentHeader, AgentReply, Button, IconButton, Thinking } from '@vizier/design'
import { useEffect, useReducer, useRef } from 'react'
import { ApiError } from '../../api/client'
import { askSideChat } from '../../api/endpoints/chat'
import { initialSideChat, mapAskError, refusal, sideChatReducer } from '../../lib/sideChat'
import { useThinkingWord } from '../../lib/thinkingWords'
import { Markdown } from '../shared/Markdown'

type Props = {
    open: boolean
    onClose: () => void
    agentId: string
    agentName: string
    /** The model the agent answers with, shown read-only. */
    modelName?: string
    channelId: string
    topicId: string
    /** The question typed after `/btw`, asked as soon as the panel opens. */
    question?: string
}

/**
 * `/btw`: the agent's answer to one throwaway question, shown in the conversation's own
 * message style and anchored directly above the composer, so the conversation stays visible
 * and usable above it. It has no input of its own: another question is another `/btw`. The
 * answer lives in component state only; closing, switching conversation or unmounting
 * discards it, and an ask still in flight is aborted so a late answer never shows in a
 * reopened panel.
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

    const turn = state.turns[0]
    // A question the server would refuse is said so, without a request.
    const problem = question?.trim() ? refusal(question) : null
    // Waiting on the answer: the line says what the agent is doing, and changes as it waits.
    const waiting = open && !problem && turn?.answer == null && !turn?.error
    const word = useThinkingWord(waiting)

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

    // The panel belongs to one open `/btw` on one conversation: leaving either discards it.
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

    // Escape closes it from anywhere in the page, since focus stays in the composer.
    useEffect(() => {
        if (!open) return
        const onKey = (e: KeyboardEvent) => {
            if (e.key === 'Escape') onClose()
        }
        window.addEventListener('keydown', onKey)
        return () => window.removeEventListener('keydown', onKey)
    }, [open, onClose])

    if (!open) return null

    return (
        <section
            aria-label="Side chat"
            className="mb-8 flex max-h-[50vh] flex-col overflow-hidden rounded-xl border border-border-strong bg-surface/70 shadow-dialog backdrop-blur-md"
        >
            <header className="flex shrink-0 items-center gap-12 border-b border-border px-16 py-8">
                <span className="type-code font-medium text-live-text">/btw</span>
                <span className="min-w-0 flex-1 truncate text-muted" title={question?.trim()}>
                    {question?.trim()}
                </span>
                <span className="type-code text-muted">esc</span>
                <IconButton variant="ghost" size="sm" icon="close" aria-label="Close side chat" onClick={onClose} />
            </header>

            <div className="flex min-h-0 flex-1 flex-col gap-16 overflow-y-auto px-16 py-16">
                <div className="flex flex-col gap-10">
                    <AgentHeader agentId={agentId} name={agentName} time={modelName} />
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
                        <Thinking>{`${agentName} is ${word}`}</Thinking>
                    )}
                </div>
            </div>
        </section>
    )
}
