import { Badge, Notice, Select, ToggleChip } from '@vizier/design'
import { allowsMessaging, MEMORY_READ, scopeFor, scopeMode, scopeNames, toggleScope, type ScopeMode } from '../../lib/triggers'

/** The categories a person can pick by name. Others a scope already names are kept and shown. */
const LABELS: Record<string, string> = {
    [MEMORY_READ]: 'Memory · read',
    memory: 'Memory · read and write',
    web: 'Web',
    shell: 'Shell',
    secrets: 'Secrets',
    files: 'Files',
    skills: 'Skills',
    scratchpad: 'Scratchpad',
    time: 'Time',
    history: 'Earlier output',
    scheduling: 'Schedule',
    agents: 'Background work',
    subagents: 'Subagents',
    consultations: 'Consultations',
    media: 'Images',
    audio: 'Audio',
    integrations: 'Integrations',
    custom_tools: 'Managing functions',
    brave_search: 'Brave Search',
    office: 'Documents',
    discord: 'Discord',
    telegram: 'Telegram',
    webui: 'Messages',
    sessions: 'Conversations',
}

/** What a monitor's check may never call, whatever is listed. */
const NEVER_IN_A_CHECK = ['subagents', 'consultations', 'media', 'audio', 'scheduling', 'integrations', 'custom_tools', 'discord', 'telegram', 'webui', 'sessions']

export const categoryLabel = (name: string) => LABELS[name] ?? name

/** A scope as a row of tags, for reading. */
export function ScopeTags({ scope }: { scope: string[] | null }) {
    const mode = scopeMode(scope)
    if (mode === 'every') return <span className="type-small text-muted">Every tool</span>
    const names = scopeNames(scope)
    return (
        <div className="flex flex-wrap items-center gap-6">
            {mode === 'except' && <span className="type-small text-muted">Every tool except</span>}
            {names.length === 0 && <span className="type-small text-muted">No tools</span>}
            {names.map((n) => (
                <Badge key={n}>{categoryLabel(n)}</Badge>
            ))}
        </div>
    )
}

/** Pick categories from the list, keeping any name the list does not know. */
export function CategoryChips({ selected, onChange, forScript }: { selected: string[]; onChange: (next: string[]) => void; forScript?: boolean }) {
    const known = Object.keys(LABELS).filter((n) => !forScript || !NEVER_IN_A_CHECK.includes(n))
    const names = [...known, ...selected.filter((n) => !known.includes(n))]
    return (
        <div className="flex flex-wrap gap-8">
            {names.map((n) => (
                <ToggleChip
                    key={n}
                    label={categoryLabel(n)}
                    checked={selected.includes(n)}
                    onCheckedChange={(on) => onChange(on ? [...selected, n] : selected.filter((s) => s !== n))}
                />
            ))}
        </div>
    )
}

/**
 * What a trigger's runs may use. A saved list stays in its own form (only these, or all but
 * these); changing the form is the person's choice and starts that form afresh.
 */
export function ScopeEditor({ scope, onChange }: { scope: string[] | null; onChange: (next: string[] | null) => void }) {
    const mode = scopeMode(scope)
    const names = scopeNames(scope)
    return (
        <div className="flex flex-col gap-12">
            <Select<ScopeMode>
                label="Runs may use"
                className="max-w-364"
                value={mode}
                onChange={(next) => next !== mode && onChange(scopeFor(next))}
                options={[
                    { value: 'only', label: 'Only the tools chosen below' },
                    { value: 'except', label: 'Every tool except those below' },
                    { value: 'every', label: 'Every tool' },
                ]}
            />
            {mode !== 'every' && (
                <CategoryChips
                    selected={names}
                    onChange={(next) => {
                        let result = scope ?? []
                        for (const n of names.filter((n) => !next.includes(n))) result = toggleScope(result, n, false, mode)
                        for (const n of next.filter((n) => !names.includes(n))) result = toggleScope(result, n, true, mode)
                        onChange(result)
                    }}
                />
            )}
        </div>
    )
}

/** The two warnings that mean the run limit no longer bounds what reaches people (FR-016). */
export function LimitWarnings({ scope, limit, isPublic }: { scope: string[] | null; limit: number | null; isPublic?: boolean }) {
    return (
        <>
            {allowsMessaging(scope) && (
                <Notice tone="retrying" icon="alert" title="Runs can send messages">
                    The run limit no longer bounds what reaches people.{isPublic && ' Anyone who can call the address can steer a run.'}
                </Notice>
            )}
            {limit === null && (
                <Notice tone="retrying" icon="alert" title="No run limit">
                    Every important event can start a run and send you its result.
                </Notice>
            )}
        </>
    )
}
