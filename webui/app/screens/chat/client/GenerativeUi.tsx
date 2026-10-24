// Draws one saved `ui_render` call inside its reply: the spec is the call's arguments, the page
// state is the call's saved state (specs/026-generative-ui-terminal-tools). It is the host of
// json-render's `Renderer`, with our catalogue's registry, our `$computed` functions and the two
// actions json-render lacks.
import { IconButton, InteractivePage as Frame, Notice } from '@vizier/design'
import { JSONUIProvider, Renderer, useActions, type ComponentRegistry } from '@json-render/react'
import { useEffect, useMemo, useRef, useState } from 'react'
import type { ClientCallView } from '../../../api/frames'
import type { ClientCallContext } from '../../../lib/clientTools'
import { createActionHandlers } from '../../../lib/generativeUi/actions'
import { COMPUTED_FUNCTIONS } from '../../../lib/generativeUi/functions'
import { guarded } from '../../../lib/generativeUi/registry/guard'
import { registry as baseRegistry } from '../../../lib/generativeUi/registry/index'
import { createPageState, saveable, type PageState } from '../../../lib/generativeUi/state'
import { useLogicRunner } from '../../../lib/generativeUi/logic/useLogic'
import { MediaFiles } from '../../../lib/generativeUi/registry/mediaContext'
import { currentChainIsTrusted } from '../../../lib/generativeUi/trust'
import { formatIssues, validateUi, type UiSpec } from '../../../lib/generativeUi/validate'
import { topicKey } from '../../../stores/connection'
import { useTurns } from '../../../stores/turns'
import { collectWatches, startWatching, type WatchBinding } from '../../../lib/generativeUi/watch'

const registry: ComponentRegistry = guarded(baseRegistry)
const NO_FILES: never[] = []
const MAX_HEIGHT_RATIO = 0.8
const SENT_NOTICE_MS = 2500

/** The interface with its `watch` bindings taken off: those run in `Watchers`, not json-render. */
function withoutWatch(spec: UiSpec): UiSpec {
    return {
        ...spec,
        elements: Object.fromEntries(Object.entries(spec.elements).map(([key, element]) => [key, { ...element, watch: undefined }])),
    }
}

function Watchers({ spec, page }: { spec: UiSpec; page: PageState }) {
    const { execute } = useActions()
    const run = useRef(execute)
    run.current = execute
    useEffect(
        () =>
            startWatching(
                collectWatches(spec as never),
                page.store,
                page.subscribeLocal,
                (binding: WatchBinding) => run.current(binding as never)
            ),
        [spec, page]
    )
    return null
}

function fileName(title: string): string {
    return `${title.replace(/[^\w.-]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 60) || 'interface'}.json`
}

export function GenerativeUi({ call, ctx }: { call: ClientCallView; ctx: ClientCallContext }) {
    const checked = useMemo(() => validateUi({ title: call.args.title, spec: call.args.spec }), [call.args.title, call.args.spec])
    if (!checked.ok) {
        // Saved before a catalogue change, or never valid: say so rather than draw half of it.
        return <Notice tone="failed" title="This interface can't be drawn">{formatIssues(checked.issues.slice(0, 3))}</Notice>
    }
    return <Drawn call={call} ctx={ctx} spec={checked.spec} title={checked.title} />
}

function Drawn({ call, ctx, spec, title }: { call: ClientCallView; ctx: ClientCallContext; spec: UiSpec; title: string }) {
    const ctxRef = useRef(ctx)
    ctxRef.current = ctx
    const [sent, setSent] = useState(false)
    const [logicError, setLogicError] = useState<{ fn: string; message: string } | null>(null)
    const sentTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

    // One page state per drawn interface: seeded once from the spec and the state saved so far.
    const page = useMemo(
        () =>
            createPageState({
                initial: spec.state,
                saved: (call.state as Record<string, unknown> | null | undefined) ?? null,
                save: (state) => ctxRef.current.saveState(state),
            }),
        // The spec is fixed for a saved call; a different call is a different component.
        // eslint-disable-next-line react-hooks/exhaustive-deps
        [call.callId]
    )
    useEffect(() => () => page.dispose(), [page])
    useEffect(() => () => void (sentTimer.current && clearTimeout(sentTimer.current)), [])

    // State saved by another tab arrives as the call's state; merged in, not saved back.
    useEffect(() => {
        const incoming = call.state
        if (!incoming || typeof incoming !== 'object') return
        const current = JSON.stringify(saveable(page.store.getSnapshot()))
        if (JSON.stringify(saveable(incoming as Record<string, unknown>)) !== current) page.mergeRemote(incoming as Record<string, unknown>)
    }, [call.state, page])

    const runLogic = useLogicRunner(spec.logic)
    const handlers = useMemo(
        () =>
            createActionHandlers({
                store: page.store,
                runLogic,
                sendEvent: (text, data) => ctxRef.current.sendEvent(text, data),
                isTrusted: currentChainIsTrusted,
                onSent: () => {
                    setSent(true)
                    if (sentTimer.current) clearTimeout(sentTimer.current)
                    sentTimer.current = setTimeout(() => setSent(false), SENT_NOTICE_MS)
                },
                onError: (fn, message) => {
                    console.warn(`generative UI: ${fn}: ${message}`)
                    setLogicError({ fn, message })
                },
                onOk: () => setLogicError(null),
            }),
        [page, runLogic]
    )

    const renderSpec = useMemo(() => withoutWatch(spec), [spec])
    // The conversation's files, kept current by the topic's own frames: a file made later appears.
    const files = useTurns((s) => s.topics[topicKey(ctx.agentId, ctx.topicId)]?.files) ?? NO_FILES

    // Expanded, the interface covers the chat area, pinned over its rectangle.
    const wrapperRef = useRef<HTMLDivElement>(null)
    const [area, setArea] = useState<{ top: number; left: number; width: number; height: number } | null>(null)
    const measure = () => {
        const rect = wrapperRef.current?.closest('[data-chat-area]')?.getBoundingClientRect()
        return rect
            ? { top: rect.top, left: rect.left, width: rect.width, height: window.innerHeight - rect.top }
            : { top: 0, left: 0, width: window.innerWidth, height: window.innerHeight }
    }
    const fullscreen = area !== null
    useEffect(() => {
        if (!fullscreen) return
        const onResize = () => setArea(measure())
        const onKey = (e: KeyboardEvent) => e.key === 'Escape' && setArea(null)
        window.addEventListener('resize', onResize)
        window.addEventListener('keydown', onKey)
        return () => {
            window.removeEventListener('resize', onResize)
            window.removeEventListener('keydown', onKey)
        }
    }, [fullscreen])

    const download = () => {
        const blob = new Blob([JSON.stringify({ title, spec }, null, 2)], { type: 'application/json' })
        const url = URL.createObjectURL(blob)
        const link = document.createElement('a')
        link.href = url
        link.download = fileName(title)
        link.click()
        setTimeout(() => URL.revokeObjectURL(url), 0)
    }

    return (
        <div ref={wrapperRef} className={fullscreen ? 'fixed z-40 bg-ground p-16' : undefined} style={area ?? undefined}>
            <Frame
                title={title}
                label="interface"
                onFullscreen={() => setArea(fullscreen ? null : measure())}
                actions={
                    <>
                        {sent && (
                            <span role="status" className="type-mono-xs text-live-text">
                                Sent to the agent
                            </span>
                        )}
                        <IconButton variant="ghost" size="sm" icon="download" aria-label="Download the interface as a JSON file" onClick={download} />
                    </>
                }
            >
                <div
                    role="region"
                    aria-label={title}
                    className="flex min-w-0 flex-col gap-12 overflow-auto p-16"
                    style={{ maxHeight: fullscreen ? '100%' : `${Math.round(window.innerHeight * MAX_HEIGHT_RATIO)}px` }}
                >
                    <MediaFiles.Provider value={files}>
                        <JSONUIProvider registry={registry} store={page.store} handlers={handlers} functions={COMPUTED_FUNCTIONS}>
                            <Watchers spec={spec} page={page} />
                            <Renderer spec={renderSpec} registry={registry} />
                        </JSONUIProvider>
                    </MediaFiles.Provider>
                    {logicError && (
                        <p role="alert" className="type-mono-xs text-failed-text">
                            {logicError.fn}: {logicError.message}
                        </p>
                    )}
                </div>
            </Frame>
        </div>
    )
}
