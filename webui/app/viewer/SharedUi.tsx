// One interactive page in a shared copy. Revalidated here with the catalogue (the document is
// hostile input), drawn by the same registry as the chat, with images read from the media map.
// The page keeps its own state and `run` logic; `sendMessage` is inert and says so.
import { InteractivePage as Frame, Notice } from '@vizier/design'
import { JSONUIProvider, Renderer, useActions, type ComponentRegistry } from '@json-render/react'
import { useEffect, useMemo, useRef, useState } from 'react'
import type { SessionFileRecord } from '../api/frames'
import { COMPUTED_FUNCTIONS } from '../lib/generativeUi/functions'
import { useLogicRunner } from '../lib/generativeUi/logic/useLogic'
import { guarded } from '../lib/generativeUi/registry/guard'
import { registry as baseRegistry } from '../lib/generativeUi/registry/index'
import { MediaFiles } from '../lib/generativeUi/registry/mediaContext'
import { createPageState, type PageState } from '../lib/generativeUi/state'
import { formatIssues, validateUi, type UiSpec } from '../lib/generativeUi/validate'
import { collectWatches, startWatching, type WatchBinding } from '../lib/generativeUi/watch'
import { createViewerHandlers, NOT_IN_A_COPY } from '../lib/share/actions'
import type { UiPart } from '../lib/share/schema'

const registry: ComponentRegistry = guarded(baseRegistry)
const NOTICE_MS = 3000

function withoutWatch(spec: UiSpec): UiSpec {
    return { ...spec, elements: Object.fromEntries(Object.entries(spec.elements).map(([k, e]) => [k, { ...e, watch: undefined }])) }
}

function Watchers({ spec, page }: { spec: UiSpec; page: PageState }) {
    const { execute } = useActions()
    const run = useRef(execute)
    run.current = execute
    useEffect(() => startWatching(collectWatches(spec as never), page.store, page.subscribeLocal, (b: WatchBinding) => run.current(b as never)), [spec, page])
    return null
}

export function SharedUi({ part, files }: { part: UiPart; files: readonly SessionFileRecord[] }) {
    const checked = useMemo(() => validateUi({ title: part.title, spec: part.spec }), [part.title, part.spec])
    if (!checked.ok) return <Notice tone="failed" title="This interactive page can't be shown">{formatIssues(checked.issues.slice(0, 3))}</Notice>
    return <Drawn part={part} files={files} spec={checked.spec} title={checked.title} />
}

function Drawn({ part, files, spec, title }: { part: UiPart; files: readonly SessionFileRecord[]; spec: UiSpec; title: string }) {
    const [blocked, setBlocked] = useState(false)
    const [logicError, setLogicError] = useState<{ fn: string; message: string } | null>(null)
    const timer = useRef<ReturnType<typeof setTimeout> | null>(null)
    // Local only: nothing is saved anywhere.
    const page = useMemo(
        () => createPageState({ initial: spec.state, saved: (part.state as Record<string, unknown> | null | undefined) ?? null, save: () => undefined }),
        // eslint-disable-next-line react-hooks/exhaustive-deps
        [part]
    )
    useEffect(() => () => page.dispose(), [page])
    useEffect(() => () => void (timer.current && clearTimeout(timer.current)), [])

    const runLogic = useLogicRunner(spec.logic)
    const handlers = useMemo(
        () =>
            createViewerHandlers({
                store: page.store,
                runLogic,
                onBlocked: () => {
                    setBlocked(true)
                    if (timer.current) clearTimeout(timer.current)
                    timer.current = setTimeout(() => setBlocked(false), NOTICE_MS)
                },
                onError: (fn, message) => setLogicError({ fn, message }),
                onOk: () => setLogicError(null),
            }),
        [page, runLogic]
    )
    const renderSpec = useMemo(() => withoutWatch(spec), [spec])

    return (
        <Frame title={title} label="interactive page · copy" actions={blocked ? <span role="status" className="type-mono-xs text-retrying">{NOT_IN_A_COPY}</span> : undefined}>
            <div role="region" aria-label={title} className="flex min-w-0 flex-col gap-12 overflow-auto p-16">
                <MediaFiles.Provider value={files}>
                    <JSONUIProvider registry={registry} store={page.store} handlers={handlers} functions={COMPUTED_FUNCTIONS}>
                        <Watchers spec={spec} page={page} />
                        <Renderer spec={renderSpec} registry={registry} />
                    </JSONUIProvider>
                </MediaFiles.Provider>
                {logicError && <p role="alert" className="type-mono-xs text-failed-text">{logicError.fn}: {logicError.message}</p>}
            </div>
        </Frame>
    )
}
