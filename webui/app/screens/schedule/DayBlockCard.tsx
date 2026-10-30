import { StatusDot, type StatusState } from '@vizier/design'
import { useNavigate } from 'react-router'
import { useHere } from '../../lib/returnTo'
import { clock, type ChipState, type Moment } from '../../lib/schedule'
import { stateWords, taskHref } from './helpers'

const DOT: Record<ChipState, StatusState> = {
    answered: 'answered',
    running: 'running',
    'no-response': 'no-response',
    interrupted: 'interrupted',
    upcoming: 'not-run',
    past: 'not-run',
    paused: 'disabled',
}

const LOOK: Record<ChipState, string> = {
    answered: 'border-transparent bg-raised',
    running: 'border-live bg-raised',
    'no-response': 'border-transparent bg-retrying-bg',
    interrupted: 'border-transparent bg-raised',
    upcoming: 'border-border-strong',
    past: 'border-transparent bg-raised',
    paused: 'border-dashed border-border-strong',
}

/** One moment on the day's timeline: its time, task, and what became of it. */
export function DayBlockCard({ agentId, moment }: { agentId: string; moment: Moment }) {
    const navigate = useNavigate()
    const here = useHere()
    const { task, at, state } = moment
    return (
        <button
            type="button"
            onClick={() => navigate(taskHref(agentId, task, state), { state: here })}
            className={`flex size-full min-w-0 flex-col justify-center gap-2 rounded-md border px-10 text-left transition-colors hover:bg-pressed ${LOOK[state]}`}
        >
            <span className="flex min-w-0 items-center gap-8">
                <StatusDot state={DOT[state]} />
                <span className="type-mono-xs shrink-0 text-muted">{clock(at)}</span>
                <span className="min-w-0 flex-1 truncate type-body-medium text-fg">{task.title}</span>
                <span className={`shrink-0 type-small ${state === 'running' ? 'text-live-text' : 'text-muted'}`}>{stateWords(state)}</span>
            </span>
            <span className="truncate type-mono-xs text-faint">{task.instruction.split('\n')[0]}</span>
        </button>
    )
}
