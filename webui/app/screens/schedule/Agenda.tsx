import { ScheduleChip } from '@vizier/design'
import { useMemo } from 'react'
import { useNavigate } from 'react-router'
import type { Task } from '../../api/endpoints/tasks'
import { monthGrid } from '../../lib/schedule'
import { useHere } from '../../lib/returnTo'
import { chipRepeat, chipTime, taskHref } from './helpers'

const heading = new Intl.DateTimeFormat('en-US', { weekday: 'long', day: 'numeric', month: 'long' })

/** The month on a phone: the days that have something, one under another. */
export function Agenda({ agentId, tasks, anchor, now, onOpenDay }: { agentId: string; tasks: Task[]; anchor: Date; now: Date; onOpenDay: (key: string) => void }) {
    const navigate = useNavigate()
    const here = useHere()
    const days = useMemo(
        () => monthGrid(tasks, anchor, now).flat().filter((d) => !d.outside && d.chips.length > 0),
        [tasks, anchor, now]
    )
    return (
        <div className="flex flex-col gap-16">
            {days.map((d) => (
                <section key={d.key} aria-label={heading.format(d.date)} className="flex flex-col gap-6">
                    <button type="button" onClick={() => onOpenDay(d.key)} className="self-start type-mono-xs uppercase text-faint">
                        {heading.format(d.date)}
                    </button>
                    {d.chips.map((c) => (
                        <ScheduleChip
                            key={c.key}
                            state={c.state}
                            time={chipTime(c)}
                            title={c.task.title}
                            repeat={chipRepeat(c)}
                            onClick={() => navigate(taskHref(agentId, c.task, c.state), { state: here })}
                        />
                    ))}
                </section>
            ))}
        </div>
    )
}
