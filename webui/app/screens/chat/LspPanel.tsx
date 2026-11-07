// The language-server panel a project chat's status strip opens (spec 054), drawn from
// `specs/054-project-lsp/mockups/lsp-status-popup.html`. There is deliberately no text about
// running code here or in the open-project dialog: that is covered in the docs, by decision.
import { Button, EmptyState, Notice, Select, StatusPanel, Switch, TextField, TextLink } from '@vizier/design'
import { useEffect, useMemo, useState } from 'react'
import {
    deleteLspOverride,
    putLspOverride,
    restartLsp,
    updateProject,
    type LspServer,
    type LspView,
} from '../../api/endpoints/projects'
import {
    buildOverride,
    formatArgs,
    formatEnv,
    groupByLanguage,
    metaLine,
    rootLabel,
    type LanguageGroup,
} from '../../lib/lsp'
import { useProjects } from '../../stores/projects'
import { toast } from '../../stores/toasts'

type Props = {
    agentId: string
    projectId: string
    onClose: () => void
}

const message = (e: unknown) => (e instanceof Error ? e.message : undefined)

function StateChip({ server }: { server: LspServer }) {
    const dot =
        server.state === 'ready'
            ? 'bg-live'
            : server.state === 'starting'
              ? 'bg-live animate-shimmer'
              : server.state === 'missing'
                ? 'bg-retrying'
                : 'bg-failed'
    const label =
        server.state === 'starting'
            ? `Starting${server.detail ? ` · ${server.detail}` : ''}`
            : server.state === 'ready'
              ? 'Ready'
              : server.state === 'missing'
                ? 'Missing'
                : 'Failed'
    return (
        <span className="inline-flex shrink-0 items-center gap-6 type-small text-muted">
            <span aria-hidden className={`size-6 rounded-pill ${dot}`} />
            {label}
        </span>
    )
}

/** The fields of one override, for Configure and for Add override. */
function OverrideForm({
    idPrefix,
    placeholder,
    initial,
    language,
    languageName,
    hasOverride,
    busy,
    onSave,
    onReset,
    onCancel,
}: {
    idPrefix: string
    placeholder: string
    initial: { command: string; args: string; env: string }
    language?: React.ReactNode
    languageName: string
    hasOverride: boolean
    busy: boolean
    onSave: (override: ReturnType<typeof buildOverride>) => void
    onReset?: () => void
    onCancel?: () => void
}) {
    const [command, setCommand] = useState(initial.command)
    const [args, setArgs] = useState(initial.args)
    const [env, setEnv] = useState(initial.env)
    const [error, setError] = useState<string | null>(null)

    const save = () => {
        const built = buildOverride(command, args, env)
        if ('error' in built) {
            setError(built.error)
            return
        }
        setError(null)
        onSave(built)
    }

    return (
        <form
            className="mt-8 flex flex-col gap-10 rounded-lg border border-border bg-raised p-12"
            onSubmit={(e) => {
                e.preventDefault()
                save()
            }}
        >
            {language}
            <TextField
                id={`${idPrefix}-command`}
                label="Command"
                mono
                value={command}
                placeholder={placeholder}
                spellCheck={false}
                autoComplete="off"
                invalid={!!error && !command.trim()}
                onChange={(e) => setCommand(e.target.value)}
            />
            <div className="grid grid-cols-2 gap-10">
                <TextField
                    id={`${idPrefix}-args`}
                    label="Arguments"
                    value={args}
                    placeholder="--stdio"
                    spellCheck={false}
                    autoComplete="off"
                    onChange={(e) => setArgs(e.target.value)}
                />
                <TextField
                    id={`${idPrefix}-env`}
                    label="Environment"
                    value={env}
                    placeholder="KEY=value"
                    spellCheck={false}
                    autoComplete="off"
                    onChange={(e) => setEnv(e.target.value)}
                />
            </div>
            {error && (
                <p role="alert" className="type-small text-failed-text">
                    {error}
                </p>
            )}
            <div className="flex flex-wrap items-center gap-8">
                <span className="min-w-0 flex-1 type-small text-faint">
                    Saving restarts {languageName} servers in this project. Only people who manage the project can
                    change this.
                </span>
                {onCancel && (
                    <Button type="button" variant="ghost" size="sm" onClick={onCancel} disabled={busy}>
                        Cancel
                    </Button>
                )}
                {hasOverride && onReset && (
                    <Button type="button" variant="secondary" size="sm" onClick={onReset} disabled={busy}>
                        Use default
                    </Button>
                )}
                <Button type="submit" size="sm" loading={busy}>
                    {onCancel ? 'Save' : 'Save and restart'}
                </Button>
            </div>
        </form>
    )
}

function LanguageBlock({
    group,
    view,
    open,
    busy,
    onToggle,
    onRestart,
    onSave,
    onReset,
}: {
    group: LanguageGroup
    view: LspView
    open: boolean
    busy: boolean
    onToggle: () => void
    onRestart: (root?: string) => void
    onSave: (override: ReturnType<typeof buildOverride>) => void
    onReset: () => void
}) {
    const override = view.overrides?.find((o) => o.language === group.id)
    const [copied, setCopied] = useState(false)
    const copy = (text: string) => {
        void navigator.clipboard?.writeText(text).then(() => {
            setCopied(true)
            setTimeout(() => setCopied(false), 1200)
        })
    }
    const binary = group.command.split(' ')[0]

    return (
        <section className="flex flex-col gap-4 border-b border-border py-10 last:border-b-0">
            <div className="flex items-center gap-8">
                <span className="type-ui-strong text-fg">{group.name}</span>
                <span className="min-w-0 flex-1 truncate type-mono-xs text-faint" title={group.command}>
                    {group.command}
                </span>
                {group.overridden && (
                    <span className="rounded-pill border border-border-strong px-6 type-mono-xs text-muted">override</span>
                )}
                {view.can_manage && (
                    <TextLink aria-expanded={open} onClick={onToggle}>
                        Configure
                    </TextLink>
                )}
            </div>

            {group.missing ? (
                <Notice
                    tone="retrying"
                    title={`${binary} isn't installed`}
                    actions={
                        <>
                            {group.missing.install_hint && (
                                <TextLink onClick={() => copy(group.missing!.install_hint)}>
                                    {copied ? 'Copied' : 'Copy'}
                                </TextLink>
                            )}
                            {view.can_manage && (
                                <TextLink onClick={() => onRestart()} disabled={busy}>
                                    Check again
                                </TextLink>
                            )}
                        </>
                    }
                >
                    The agent has been told too, and edits to these files aren't checked.
                    {group.missing.install_hint && (
                        <code className="mt-4 block truncate type-mono-xs text-fg">{group.missing.install_hint}</code>
                    )}
                </Notice>
            ) : (
                group.roots.map((root) => (
                    <div key={root.root} className="flex flex-col gap-4">
                        <div className="flex items-center gap-8">
                            <span className="min-w-0 flex-1 truncate type-mono-sm text-fg" title={root.root}>
                                {rootLabel(root.root)}
                            </span>
                            <StateChip server={root} />
                        </div>
                        {root.state === 'failed' && (
                            <Notice
                                tone="failed"
                                actions={
                                    view.can_manage ? (
                                        <TextLink onClick={() => onRestart(root.root)} disabled={busy}>
                                            Restart
                                        </TextLink>
                                    ) : undefined
                                }
                            >
                                {root.reason}
                            </Notice>
                        )}
                        {root.state === 'missing' && root.install_hint && (
                            <code className="truncate type-mono-xs text-muted">{root.install_hint}</code>
                        )}
                    </div>
                ))
            )}

            {open && (
                <OverrideForm
                    idPrefix={`lsp-${group.id}`}
                    placeholder={binary}
                    languageName={group.name}
                    initial={{
                        command: override?.command ?? '',
                        args: override ? formatArgs(override.args) : '',
                        env: override ? formatEnv(override.env) : '',
                    }}
                    hasOverride={!!override}
                    busy={busy}
                    onSave={onSave}
                    onReset={onReset}
                />
            )}
        </section>
    )
}

export function LspPanel({ agentId, projectId, onClose }: Props) {
    const view = useProjects((s) => s.lsp[projectId])
    const [expanded, setExpanded] = useState<string | null>(null)
    const [adding, setAdding] = useState(false)
    const [addLanguage, setAddLanguage] = useState<string>('')
    const [busy, setBusy] = useState(false)

    useEffect(() => useProjects.getState().watchLsp(agentId, projectId), [agentId, projectId])

    const groups = useMemo(() => (view ? groupByLanguage(view) : []), [view])

    if (!view) {
        return (
            <StatusPanel title="Language servers" onClose={onClose}>
                <EmptyState kind="empty" icon="code" title="Loading" message="Reading the language servers." />
            </StatusPanel>
        )
    }

    const store = useProjects.getState()
    const run = async (work: () => Promise<LspView | null | undefined | void>, failure: string) => {
        setBusy(true)
        try {
            const next = await work()
            if (next) store.setLsp(projectId, next)
            else await store.loadLsp(agentId, projectId)
        } catch (e) {
            toast.error(failure, message(e))
        } finally {
            setBusy(false)
        }
    }

    const toggle = (enabled: boolean) =>
        run(async () => {
            await updateProject(agentId, projectId, { lsp_enabled: enabled })
            void store.refresh(agentId)
            setExpanded(null)
            setAdding(false)
        }, "Couldn't change language servers")

    const save = (language: string, built: ReturnType<typeof buildOverride>) => {
        if ('error' in built) return
        void run(async () => {
            const next = await putLspOverride(agentId, projectId, language, built.override)
            setExpanded(null)
            setAdding(false)
            return next
        }, "Couldn't save the override")
    }

    const reset = (language: string) =>
        run(async () => {
            const next = await deleteLspOverride(agentId, projectId, language)
            setExpanded(null)
            return next
        }, "Couldn't remove the override")

    const restart = (language: string, root?: string) =>
        run(() => restartLsp(agentId, projectId, language, root), "Couldn't restart the server")

    const languages = view.languages ?? []
    const target = languages.find((l) => l.id === (addLanguage || languages[0]?.id))
    const running = view.servers.filter((s) => s.state === 'ready' || s.state === 'starting').length

    return (
        <StatusPanel
            title="Language servers"
            meta={metaLine(view)}
            onClose={onClose}
            headerAction={
                view.can_manage ? (
                    <Switch
                        checked={view.enabled}
                        disabled={busy}
                        aria-label="Language servers"
                        onCheckedChange={(on) => void toggle(on)}
                    />
                ) : undefined
            }
            footer={
                view.enabled ? (
                    <>
                        <span className="type-small text-faint">
                            {running} of {view.capacity.max} server slots in use · stops after 10 min idle
                        </span>
                        {view.can_manage && !adding && languages.length > 0 && (
                            <TextLink onClick={() => setAdding(true)}>Add override</TextLink>
                        )}
                    </>
                ) : undefined
            }
        >
            {!view.can_manage && (
                <p className="pb-8 type-small text-faint">Someone who manages this project controls these settings.</p>
            )}
            {!view.enabled ? (
                <EmptyState
                    kind="empty"
                    icon="code"
                    title="Off for this project"
                    message="The agent uses text search and has no diagnostics after edits."
                />
            ) : groups.length === 0 && !adding ? (
                <EmptyState
                    kind="empty"
                    icon="code"
                    title="No servers running"
                    message="One starts when the agent opens a file in a supported language."
                />
            ) : (
                groups.map((group) => (
                    <LanguageBlock
                        key={group.id}
                        group={group}
                        view={view}
                        open={expanded === group.id}
                        busy={busy}
                        onToggle={() => setExpanded(expanded === group.id ? null : group.id)}
                        onRestart={(root) => void restart(group.id, root)}
                        onSave={(built) => save(group.id, built)}
                        onReset={() => void reset(group.id)}
                    />
                ))
            )}
            {adding && target && (
                <OverrideForm
                    key={target.id}
                    idPrefix="lsp-add"
                    placeholder="/path/to/server"
                    languageName={target.name}
                    initial={{
                        command: view.overrides?.find((o) => o.language === target.id)?.command ?? '',
                        args: '',
                        env: '',
                    }}
                    language={
                        <Select
                            label="Language"
                            value={target.id}
                            onChange={setAddLanguage}
                            options={languages.map((l) => ({ value: l.id, label: l.name }))}
                        />
                    }
                    hasOverride={false}
                    busy={busy}
                    onSave={(built) => save(target.id, built)}
                    onCancel={() => setAdding(false)}
                />
            )}
        </StatusPanel>
    )
}
