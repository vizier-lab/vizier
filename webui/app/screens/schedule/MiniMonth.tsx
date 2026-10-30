import { IconButton } from '@vizier/design'
import { dayKey, monthWeeks } from '../../lib/schedule'

const LETTERS = ['M', 'T', 'W', 'T', 'F', 'S', 'S']
const title = new Intl.DateTimeFormat('en-US', { month: 'long', year: 'numeric' })
const full = new Intl.DateTimeFormat('en-US', { weekday: 'long', day: 'numeric', month: 'long' })

type Props = {
    anchor: Date
    selected: string
    today: string
    /** Days that have something scheduled. */
    marked: Set<string>
    onSelect: (key: string) => void
    onStepMonth: (by: -1 | 1) => void
}

/** A small month for jumping to a day. */
export function MiniMonth({ anchor, selected, today, marked, onSelect, onStepMonth }: Props) {
    const weeks = monthWeeks(anchor)
    return (
        <section aria-label="Jump to a day" className="flex flex-col gap-8 rounded-xl border border-border bg-surface p-16">
            <header className="flex items-center justify-between">
                <h2 className="type-heading text-fg">{title.format(anchor)}</h2>
                <span className="flex gap-4">
                    <IconButton icon="chevron-left" variant="ghost" size="sm" aria-label="Previous month" onClick={() => onStepMonth(-1)} />
                    <IconButton icon="chevron-right" variant="ghost" size="sm" aria-label="Next month" onClick={() => onStepMonth(1)} />
                </span>
            </header>
            <div className="grid grid-cols-7 gap-y-2">
                {LETTERS.map((l, i) => (
                    <span key={i} aria-hidden className="type-mono-xs py-4 text-center text-faint">
                        {l}
                    </span>
                ))}
                {weeks.flat().map((d) => {
                    const key = dayKey(d)
                    const on = key === selected
                    const outside = d.getMonth() !== anchor.getMonth()
                    return (
                        <button
                            key={key}
                            type="button"
                            aria-label={full.format(d)}
                            aria-pressed={on}
                            aria-current={key === today ? 'date' : undefined}
                            onClick={() => onSelect(key)}
                            className={`relative mx-auto flex size-28 items-center justify-center rounded-pill type-mono-sm transition-colors ${
                                on ? 'bg-ink text-ground' : `hover:bg-raised ${outside ? 'text-unlit' : key === today ? 'text-fg' : 'text-soft'}`
                            }`}
                        >
                            {d.getDate()}
                            {marked.has(key) && !outside && !on && <span aria-hidden className="absolute bottom-2 size-3 rounded-pill bg-faint" />}
                        </button>
                    )
                })}
            </div>
        </section>
    )
}

