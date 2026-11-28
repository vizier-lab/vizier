import { Button, CodeBlock, Dialog, EmptyState, FilterTabs, KeyValueList, Notice, PanelHeader, ProgressBar, Skeleton, Switch } from '@vizier/design'
import { useCallback, useEffect, useState, type ReactNode } from 'react'
import { useNavigate, useParams, useSearchParams } from 'react-router'
import { ApiError } from '../api/client'
import {
    approveTrigger, deleteTrigger, getTrigger, replaceTriggerSecret, testTrigger, updateTrigger,
    type CheckOutcome, type Trigger, type TriggerKind,
} from '../api/endpoints/triggers'
import { canManageTriggers, canPublishWebhooks } from '../lib/access'
import { absoluteTime, relativeTime } from '../lib/format'
import { highlightCode } from '../lib/highlight'
import { useCarry, useReturnTo } from '../lib/returnTo'
import { fieldValues, intervalText, kindOf, settingsFrom, stopped, takeSecret } from '../lib/triggers'
import { ModelSelect } from '../screens/agents/ModelSelect'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { Markdown } from '../screens/shared/Markdown'
import { PageFrame } from '../screens/shared/PageFrame'
import { Panes } from '../screens/shared/Panes'
import { FadeRule } from '../screens/shared/SideCard'
import { Events, Runs } from '../screens/triggers/Activity'
import { SecretDialog, webhookAddress } from '../screens/triggers/SecretDialog'
import { CategoryChips, LimitWarnings, ScopeTags } from '../screens/triggers/ToolAccess'
import { useDestinations, useTriggerKinds } from '../screens/triggers/useTrigger'
import { useAgents } from '../stores/agents'
import { useSession } from '../stores/session'
import { toast } from '../stores/toasts'
import { useTriggers } from '../stores/triggers'

export default function TriggerRoute() {
    const { agentId = '', slug = '' } = useParams()
    return <TriggerPage key={`${agentId}/${slug}`} agentId={agentId} slug={slug} />
}

const reason = (e: unknown) => (e instanceof ApiError || e instanceof Error ? e.message : undefined)

/** One trigger: what happened to its events, and what it is set to do. */
function TriggerPage({ agentId, slug }: { agentId: string; slug: string }) {
    const navigate = useNavigate()
    const carry = useCarry()
    const returnTo = useReturnTo(`/${agentId}/triggers`)
    const [params, setParams] = useSearchParams()
    const agent = useAgents((s) => s.agents.find((a) => a.agent_id === agentId))
    const manages = !!agent && canManageTriggers(agent)
    const publishes = !!agent && canPublishWebhooks(agent)
    const version = useTriggers((s) => s.version[agentId] ?? 0)
    const kinds = useTriggerKinds()

    const [trigger, setTrigger] = useState<Trigger | null>(null)
    const [missing, setMissing] = useState(false)
    // Held in memory by the page that made it; taken once, so a reload has nothing to show.
    const [secret, setSecret] = useState<string | null>(() => takeSecret(agentId, slug))
    const [confirm, setConfirm] = useState<'delete' | 'secret' | null>(null)
    const [busy, setBusy] = useState<string | null>(null)
    const [outcome, setOutcome] = useState<CheckOutcome | null>(null)

    useEffect(() => {
        let live = true
        getTrigger(agentId, slug).then(
            (t) => live && setTrigger(t),
            (e) => {
                if (!live) return
                if (e instanceof ApiError && e.status === 404) setMissing(true)
                else toast.error("Couldn't load the trigger", reason(e))
            }
        )
        return () => {
            live = false
        }
    }, [agentId, slug, version])

    /** Do one thing to the trigger, say why if it fails, and read everything again. */
    const act = useCallback(
        async (name: string, failed: string, work: () => Promise<void>) => {
            setBusy(name)
            try {
                await work()
            } catch (e) {
                toast.error(failed, reason(e))
            } finally {
                setBusy(null)
                setConfirm(null)
                useTriggers.getState().changed(agentId)
            }
        },
        [agentId]
    )

    const back = { href: returnTo, label: 'Triggers' }
    if (missing) {
        return (
            <PageFrame title={slug} back={back}>
                <EmptyState kind="error" title="This trigger isn’t there" message="It may have been deleted. Every trigger is listed on Triggers." />
            </PageFrame>
        )
    }
    if (!trigger || !kinds) {
        return (
            <PageFrame title={slug} back={back}>
                <Skeleton lines={8} />
            </PageFrame>
        )
    }

    const t = trigger
    const kind = kindOf(kinds, t.kind)
    const waiting = !!t.awaiting_approval
    const locked = !!kind?.public && !publishes
    const needsModel = waiting && !!t.triage && !(t.triage.provider && t.triage.model)
    const run = Number(params.get('run')) || null
    // A webhook is read by its events, which say why each did or didn't run.
    const tab = params.get('tab') ?? (run ? 'runs' : t.address_path ? 'events' : 'runs')
    const show = (next: Record<string, string>) => setParams(next, { replace: true })

    const test = () => act('test', "Couldn't test it", async () => setOutcome(await testTrigger(agentId, slug)))
    const toggle = (on: boolean) => act('active', "Couldn't change the trigger", async () => void (await updateTrigger(agentId, slug, { is_active: on })))

    const edit = () => navigate(`/${agentId}/triggers/${encodeURIComponent(slug)}/edit`, { state: carry })
    const interval = t.monitor ? Number(t.settings.interval_secs ?? kind?.listing.fields.find((f) => f.key === 'interval_secs')?.default) : null

    return (
        <PageFrame
            fill
            back={back}
            title={t.title}
            meta={[(kind?.listing.name ?? t.kind).toLowerCase(), t.slug, interval ? intervalText(interval) : null, t.last_fired_at ? `last fired ${relativeTime(t.last_fired_at)}` : 'never run'].filter(Boolean).join(' · ')}
            actions={
                manages && (
                    <>
                        {!waiting && (
                            <label className="flex items-center gap-8 type-ui text-muted">
                                <Switch checked={t.is_active} disabled={busy !== null || (locked && !t.is_active)} aria-label="Active" onCheckedChange={(on) => void toggle(on)} />
                                {t.is_active ? 'Active' : 'Paused'}
                            </label>
                        )}
                        <Button variant="ghost" icon="trash" onClick={() => setConfirm('delete')}>
                            Delete
                        </Button>
                        {t.monitor && !waiting && (
                            <Button icon="play" loading={busy === 'test'} onClick={() => void test()}>
                                Test
                            </Button>
                        )}
                        <Button variant={waiting ? 'secondary' : 'primary'} icon="pencil" onClick={edit}>
                            Edit
                        </Button>
                        {waiting && <ApproveButton trigger={t} locked={locked} needsModel={needsModel} busy={busy === 'approve'} act={act} onSecret={setSecret} />}
                    </>
                )
            }
        >
            <Panes paneWidth={360} paneLabel="Settings" pane={<Settings trigger={t} kind={kind} manages={manages} publishes={publishes} onReplaceSecret={() => setConfirm('secret')} />}>
                {waiting && (
                    <Notice tone="retrying" icon="alert" title="This trigger does nothing until a person approves it">
                        {'agent' in t.requester ? 'The agent set it up on its own.' : 'The agent set it up.'} It was created {relativeTime(t.created_at)}. Read what it does and what it may use before you approve.
                        {locked && manages && ' Approving a webhook needs the public webhooks permission.'}
                    </Notice>
                )}
                {stopped(t) && (
                    <Notice
                        tone="failed"
                        title={`Stopped after ${t.monitor!.failures} failed checks`}
                        actions={
                            manages && (
                                // Saving a monitor, even unchanged, starts its checks again.
                                <Button size="sm" loading={busy === 'restart'} onClick={() => void act('restart', "Couldn't restart it", async () => void (await updateTrigger(agentId, slug, {})))}>
                                    Restart
                                </Button>
                            )
                        }
                    >
                        {t.monitor!.last_error && <span className="block break-words type-mono-sm">{t.monitor!.last_error}</span>}
                        Nothing fires while it is stopped. Restarting or editing it starts the checks again.
                    </Notice>
                )}
                {t.needs_attention && (
                    <Notice tone="retrying" icon="alert" title="Needs attention" actions={manages && <Button size="sm" onClick={edit}>Edit</Button>}>
                        {t.needs_attention}
                    </Notice>
                )}
                {outcome && <TestResult outcome={outcome} onClose={() => setOutcome(null)} />}

                {waiting && (
                    <section className="flex max-w-[72ch] flex-col gap-8">
                        <PanelHeader title="Instruction" level={2} />
                        <Markdown size="document">{t.instruction}</Markdown>
                    </section>
                )}
                <Script trigger={t} kind={kind} editable={waiting && manages} busy={busy} onTest={() => void test()} act={act} first={waiting} />

                {!waiting && (
                    <>
                        <FilterTabs
                            aria-label="What happened"
                            className="self-start"
                            value={tab}
                            onChange={(v) => show({ tab: v })}
                            options={[
                                { value: 'runs', label: 'Runs' },
                                { value: 'events', label: 'Events' },
                            ]}
                        />
                        {tab === 'events' ? (
                            <Events agentId={agentId} slug={slug} version={version} onRun={(id) => show({ run: String(id) })} />
                        ) : (
                            <Runs agentId={agentId} slug={slug} version={version} selected={run} onSelect={(id) => show({ run: String(id) })} />
                        )}
                    </>
                )}
            </Panes>

            <ConfirmDialog
                open={confirm === 'delete'}
                title={`Delete ${t.title}?`}
                removes={['The trigger and its address', 'Its events and run history']}
                confirmLabel="Delete trigger"
                onConfirm={() =>
                    void act('delete', "Couldn't delete the trigger", async () => {
                        await deleteTrigger(agentId, slug)
                        toast.success('Trigger deleted')
                        navigate(returnTo)
                    })
                }
                onCancel={() => setConfirm(null)}
            />
            <ConfirmDialog
                open={confirm === 'secret'}
                title="Replace the secret?"
                message="The current secret stops working at once. Whatever calls this webhook needs the new one."
                confirmLabel="Replace secret"
                onConfirm={() => void act('secret', "Couldn't replace the secret", async () => setSecret((await replaceTriggerSecret(agentId, slug)).secret ?? null))}
                onCancel={() => setConfirm(null)}
            />
            <SecretDialog secret={secret} path={t.address_path} onClose={() => setSecret(null)} />
        </PageFrame>
    )
}

type Act = (name: string, failed: string, work: () => Promise<void>) => Promise<void>

/** Approve, choosing the triage model first where the trigger has triage. */
function ApproveButton({ trigger, locked, needsModel, busy, act, onSecret }: { trigger: Trigger; locked: boolean; needsModel: boolean; busy: boolean; act: Act; onSecret: (s: string | null) => void }) {
    const choosesModel = useSession((s) => s.user?.permissions.includes('model_override') ?? false)
    const [choosing, setChoosing] = useState(false)
    const [model, setModel] = useState({ provider: '', model: '' })
    const approve = (chosen?: [string, string]) =>
        act('approve', "Couldn't approve it", async () => {
            const done = await approveTrigger(trigger.agent_id, trigger.slug, chosen)
            setChoosing(false)
            toast.success('Trigger approved')
            onSecret(done.secret ?? null)
        })
    return (
        <>
            <Button
                variant="primary"
                icon="check"
                loading={busy}
                disabled={locked || (needsModel && !choosesModel)}
                title={needsModel && !choosesModel ? 'Approving it needs someone who may choose models' : undefined}
                onClick={() => (needsModel ? setChoosing(true) : void approve())}
            >
                Approve
            </Button>
            <Dialog open={choosing} size="lg" title="Choose the triage model" message="It judges which events are worth a run. It has no tools and no memory." confirmLabel="Approve" confirmDisabled={!model.provider || !model.model} confirmLoading={busy} onConfirm={() => void approve([model.provider, model.model])} onCancel={() => setChoosing(false)}>
                <ModelSelect compact provider={model.provider} model={model.model} onChange={(c) => setModel({ provider: c.provider, model: c.model })} />
            </Dialog>
        </>
    )
}

/** The code a trigger runs, and for one awaiting approval what that code may call. */
function Script({ trigger, kind, editable, busy, onTest, act, first }: { trigger: Trigger; kind?: TriggerKind; editable: boolean; busy: string | null; onTest: () => void; act: Act; first: boolean }) {
    const fields = kind?.listing.fields ?? []
    const code = fields.filter((f) => f.type === 'code' && typeof trigger.settings[f.key] === 'string')
    const lists = fields.filter((f) => f.type === 'tool_categories')
    const [picked, setPicked] = useState<Record<string, string[]> | null>(null)
    if (code.length === 0) return null
    const saved = Object.fromEntries(lists.map((f) => [f.key, (trigger.settings[f.key] as string[] | undefined) ?? []]))
    return (
        <section className={`flex flex-col gap-12 ${first ? '' : 'order-last'}`}>
            {code.map((f) => {
                const text = String(trigger.settings[f.key])
                return <CodeBlock key={f.key} filename={f.label} lineNumbers code={text} tokens={highlightCode(text, 'python')} />
            })}
            {editable &&
                lists.map((f) => (
                    <div key={f.key} className="flex flex-col gap-8">
                        <PanelHeader title={f.label} level={3} />
                        <p className="type-small-para text-muted">An agent’s script starts with reading memory only. Add what it needs, then test it.</p>
                        <CategoryChips forScript selected={picked?.[f.key] ?? saved[f.key]} onChange={(next) => setPicked({ ...saved, ...picked, [f.key]: next })} />
                    </div>
                ))}
            {editable && (
                <div className="flex flex-wrap items-center gap-12">
                    {picked && (
                        <Button
                            loading={busy === 'tools'}
                            onClick={() =>
                                void act('tools', "Couldn't change what it may call", async () => {
                                    const settings = settingsFrom(fields, { ...fieldValues(fields, trigger.settings), ...picked }, trigger.settings)
                                    await updateTrigger(trigger.agent_id, trigger.slug, { settings })
                                    setPicked(null)
                                })
                            }
                        >
                            Save tools
                        </Button>
                    )}
                    <Button icon="play" loading={busy === 'test'} disabled={!!picked} onClick={onTest}>
                        Test script
                    </Button>
                    <span className="type-small text-faint">Runs it once without firing. Its tool calls really happen.</span>
                </div>
            )}
        </section>
    )
}

function TestResult({ outcome, onClose }: { outcome: CheckOutcome; onClose: () => void }) {
    const value = outcome.value === undefined || outcome.value === null ? 'None' : JSON.stringify(outcome.value, null, 2)
    return (
        <section aria-label="Test result" className="flex flex-col gap-8">
            <Notice tone={outcome.error ? 'failed' : 'info'} title={outcome.error ? 'The test failed' : 'The test ran. Nothing fired and no state was saved.'} actions={<Button size="sm" variant="ghost" onClick={onClose}>Close</Button>}>
                {outcome.error}
                {!!outcome.calls?.length && <span className="block">Called: {outcome.calls.join(', ')}</span>}
            </Notice>
            {!outcome.error && <CodeBlock language="json" code={value} />}
        </section>
    )
}

function Section({ title, action, children }: { title: string; action?: ReactNode; children: ReactNode }) {
    return (
        <>
            <section className="flex flex-col gap-8">
                <div className="flex items-center gap-8">
                    <PanelHeader title={title} level={3} className="flex-1" />
                    {action}
                </div>
                {children}
            </section>
            <FadeRule className="last:hidden" />
        </>
    )
}

function Settings({ trigger: t, kind, manages, publishes, onReplaceSecret }: { trigger: Trigger; kind?: TriggerKind; manages: boolean; publishes: boolean; onReplaceSecret: () => void }) {
    const me = useSession((s) => s.user?.user_id)
    const destinations = useDestinations(t.agent_id)
    const fields = kind?.listing.fields ?? []
    // The kind's own settings that say something: code and tool lists are shown elsewhere.
    const rows = fields.flatMap((f) => {
        const v = t.settings[f.key]
        if (f.generated || f.type === 'code' || f.type === 'tool_categories' || v === undefined || v === '') return []
        const text = Array.isArray(v) ? v.join(', ') : String(v)
        return text ? [{ label: f.label.replace(/\s*\(.*\)$/, ''), value: <span className="type-mono-sm" title={text}>{text}</span> }] : []
    })
    const generatedSecret = fields.find((f) => f.generated && f.type === 'secret')
    const tools = fields.find((f) => f.type === 'tool_categories')
    const m = t.monitor
    return (
        <>
            {!t.awaiting_approval && (
                <Section title="Instruction">
                    <Markdown>{t.instruction}</Markdown>
                </Section>
            )}
            {m && (
                <Section title="Health">
                    <KeyValueList
                        items={[
                            { label: 'Last check', value: m.last_check_at ? `${m.last_error ? 'failed · ' : ''}${relativeTime(m.last_check_at)}` : 'not yet' },
                            { label: 'Next check', value: m.failed_at ? 'none while stopped' : m.next_check_at && t.is_active && !t.awaiting_approval ? absoluteTime(m.next_check_at) : '–' },
                            { label: 'Failures in a row', value: m.failures },
                        ]}
                    />
                    {m.last_error && !m.failed_at && <p className="break-words type-mono-sm text-failed-text">{m.last_error}</p>}
                </Section>
            )}
            {t.address_path && (
                <Section title="Address" action={manages && publishes && <Button size="sm" variant="ghost" onClick={onReplaceSecret}>Replace secret</Button>}>
                    <CodeBlock code={webhookAddress(t.address_path)} />
                    {generatedSecret && <KeyValueList items={[{ label: generatedSecret.label, value: <span className="type-mono-sm">{String(t.settings[generatedSecret.key] ?? '–')}</span> }]} />}
                </Section>
            )}
            {rows.length > 0 && (
                <Section title={`${kind?.listing.name ?? 'Kind'} settings`}>
                    <KeyValueList items={rows} />
                </Section>
            )}
            <Section title="Limits">
                <KeyValueList
                    items={[
                        { label: 'Runs this hour', value: t.hourly_limit === null ? `${t.runs_last_hour} · no limit` : `${t.runs_last_hour} of ${t.hourly_limit}` },
                        { label: 'Cooldown', value: t.cooldown_secs ? `${t.cooldown_secs} s` : 'none' },
                        { label: 'While a run is going', value: t.overlap === 'drop' ? 'events are dropped' : 'events wait' },
                    ]}
                />
                {t.hourly_limit !== null && <ProgressBar compact label={`${t.runs_last_hour} of ${t.hourly_limit} runs this hour`} value={(t.runs_last_hour / t.hourly_limit) * 100} />}
            </Section>
            {t.triage && (
                <Section title="Triage">
                    <p className="type-small-para text-soft">“{t.triage.criteria}”</p>
                    <KeyValueList
                        items={[
                            { label: 'Model', value: t.triage.model ? <span className="type-mono-sm">{t.triage.provider} / {t.triage.model}</span> : 'chosen on approval' },
                            { label: 'Calls per hour', value: `up to ${t.triage.calls_per_hour}` },
                        ]}
                    />
                </Section>
            )}
            <Section title="Results go to">
                <KeyValueList
                    items={[
                        { label: 'Inbox', value: !t.delivers_to ? 'the agent’s owner' : t.delivers_to === me ? 'you' : 'whoever set it up' },
                        ...(t.destination ? [{ label: 'Also', value: destinations.find((d) => d.value === t.destination)?.label ?? 'a conversation of theirs' }] : []),
                        { label: 'Quiet summary', value: t.summary_hours ? `every ${t.summary_hours} h` : 'off' },
                    ]}
                />
            </Section>
            <Section title="Runs may use">
                <ScopeTags scope={t.tool_scope} />
                <LimitWarnings scope={t.tool_scope} limit={t.hourly_limit} isPublic={!!t.address_path} />
                {tools && !t.awaiting_approval && (
                    <>
                        <p className="type-small text-muted">{tools.label}</p>
                        <ScopeTags scope={(t.settings[tools.key] as string[] | undefined) ?? []} />
                    </>
                )}
            </Section>
        </>
    )
}
