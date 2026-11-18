// A version that froze the tab last time it ran (mockups/page-frame.html case E, FR-016a): drawn
// paused so the chat stays usable, until the person runs it anyway.

import { Button, Icon } from '@vizier/design'
import { useSyncExternalStore } from 'react'
import { isPaused, onPausedChange, runAnyway } from '../../../lib/uiPage/mounting'

/** Whether `page` v`version` is paused, and the way to run it anyway. */
export function usePaused(page: string, version: number): [boolean, () => void] {
    const paused = useSyncExternalStore(onPausedChange, () => isPaused(page, version), () => false)
    return [paused, () => runAnyway(page, version)]
}

export function PausedPage({ onRun }: { onRun: () => void }) {
    return (
        <div className="grid place-items-center gap-8 bg-surface px-16 py-40 text-center">
            <Icon name="alert" size={22} className="text-retrying" />
            <p className="m-0 max-w-[380px] type-ui text-soft">
                <b className="text-fg">This page stopped responding last time it ran.</b>
                <br />
                It is paused so the chat stays usable.
            </p>
            <Button variant="secondary" size="sm" icon="play" onClick={onRun}>
                Run anyway
            </Button>
        </div>
    )
}
