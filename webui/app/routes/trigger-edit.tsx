import { Button, EmptyState, Notice, Radio, Select, Skeleton, SwitchRow, Textarea, TextField } from '@vizier/design'
import { useEffect, useState } from 'react'
import { useNavigate, useParams, useSearchParams } from 'react-router'
import { ApiError } from '../api/client'
import { createTrigger, getTrigger, updateTrigger, type Trigger, type TriggerInput, type TriggerKind } from '../api/endpoints/triggers'
import { canManageTriggers, canPublishWebhooks } from '../lib/access'
import { useCarry, useReturnTo } from '../lib/returnTo'
import { slugOf } from '../lib/tasks'
import { fieldValues, holdSecret, kindOf, missingFields, settingsFrom, wholeNumber, type FieldValues } from '../lib/triggers'
import { useMobile } from '../lib/useMobile'
import { ModelSelect } from '../screens/agents/ModelSelect'
import { PageFrame } from '../screens/shared/PageFrame'
import { KindFields } from '../screens/triggers/KindFields'
import { LimitWarnings, ScopeEditor } from '../screens/triggers/ToolAccess'
import { useDestinations, useTriggerKinds } from '../screens/triggers/useTrigger'
import { useAgents } from '../stores/agents'
import { useSession } from '../stores/session'
import { toast } from '../stores/toasts'
import { useTriggers } from '../stores/triggers'

const FORM = 'trigger-form'

export default function TriggerEditRoute() {
    const { agentId = '', slug = '' } = useParams()
    const kinds = useTriggerKinds()
    const agent = useAgents((s) => s.agents.find((a) => a.agent_id === agentId))
    const creating = slug === 'new'
    const [trigger, setTrigger] = useState<Trigger | null>(null)
    const back = useReturnTo(`/${agentId}/triggers`)

    useEffect(() => {
        setTrigger(null)
        if (creating) return
        getTrigger(agentId, slug).then(setTrigger, (e) => toast.error("Couldn't load the trigger", e.message))
    }, [agentId, slug, creating])

    const title = creating ? 'New trigger' : 'Edit trigger'
    if (agent && !canManageTriggers(agent)) {
        return (
            <PageFrame title={title} back={{ href: back, label: 'Triggers' }}>
                <EmptyState kind="error" title="You may not change triggers" message="Ask someone who manages this agent’s triggers." />
            </PageFrame>
        )
    }
    if (!agent || !kinds || (!creating && !trigger)) {
        return (
            <PageFrame title={title} back={{ href: back, label: 'Triggers' }}>
                <Skeleton lines={10} />
            </PageFrame>
        )
    }
    return <TriggerForm key={`${agentId}/${slug}`} agentId={agentId} kinds={kinds} trigger={trigger} publishes={canPublishWebhooks(agent)} />
}

type Draft = {
    kind: string
    title: string
    slug: string
    instruction: string
    values: FieldValues
    triage: boolean
    criteria: string
    provider: string
    model: string
    calls: string
    destination: string
    summary: string
    limited: boolean
    limit: string
    cooldown: string
    overlap: 'wait' | 'drop'
    scope: string[] | null
}

function startingDraft(kinds: TriggerKind[], t: Trigger | null, asked: string | null, publishes: boolean): Draft {
    const usable = kinds.filter((k) => publishes || !k.public)
    const kind = t ? kindOf(kinds, t.kind) : (usable.find((k) => k.id === asked) ?? usable[0])
    return {
        kind: t?.kind ?? kind?.id ?? '',
        title: t?.title ?? '',
        slug: t?.slug ?? '',
        instruction: t?.instruction ?? '',
        values: fieldValues(kind?.listing.fields ?? [], t?.settings),
        triage: !!t?.triage,
        criteria: t?.triage?.criteria ?? '',
        provider: t?.triage?.provider ?? '',
        model: t?.triage?.model ?? '',
        calls: String(t?.triage?.calls_per_hour ?? 200),
        destination: t?.destination ?? '',
        summary: String(t ? (t.summary_hours ?? 0) : 5),
        limited: t ? t.hourly_limit !== null : true,
        limit: String(t?.hourly_limit ?? 20),
        cooldown: String(t?.cooldown_secs ?? 0),
        overlap: t?.overlap ?? 'wait',
        scope: t ? t.tool_scope : (kind?.default_scope ?? null),
    }
}

/** What a draft says, as the server takes it. */
function inputOf(d: Draft, kind: TriggerKind | undefined, saved: Record<string, unknown>): TriggerInput {
    return {
        title: d.title.trim(),
        instruction: d.instruction.trim(),
        settings: settingsFrom(kind?.listing.fields ?? [], d.values, saved),
        triage: d.triage
            ? { criteria: d.criteria.trim(), provider: d.provider || null, model: d.model || null, calls_per_hour: wholeNumber(d.calls) ?? 0 }
            : null,
        destination: d.destination || null,
        summary_hours: Number(d.summary) || null,
        hourly_limit: d.limited ? (wholeNumber(d.limit) ?? 0) : null,
        cooldown_secs: wholeNumber(d.cooldown) ?? 0,
        overlap: d.overlap,
        tool_scope: d.scope,
    }
}

function TriggerForm({ agentId, kinds, trigger, publishes }: { agentId: string; kinds: TriggerKind[]; trigger: Trigger | null; publishes: boolean }) {
    const navigate = useNavigate()
    const mobile = useMobile()
    const carry = useCarry()
    const fromList = useReturnTo(`/${agentId}/triggers`)
    const [params] = useSearchParams()
    const choosesModel = useSession((s) => s.user?.permissions.includes('model_override') ?? false)
    const destinations = useDestinations(agentId)
    const creating = !trigger
    const [start] = useState(() => startingDraft(kinds, trigger, params.get('kind'), publishes))
    const [d, setDraft] = useState(start)
    const [busy, setBusy] = useState(false)
    const [error, setError] = useState<string>()
    const set = (patch: Partial<Draft>) => setDraft((prev) => ({ ...prev, ...patch }))

    const kind = kindOf(kinds, d.kind)
    const fields = kind?.listing.fields ?? []
    const slug = creating ? d.slug || slugOf(d.title) : trigger.slug
    // One an agent made may wait for its model until it is approved.
    const needsModel = d.triage && !trigger?.awaiting_approval && !(d.provider && d.model)
    const problem = !kind
        ? 'Choose a kind.'
        : !d.title.trim() || !d.instruction.trim()
          ? 'It needs a title and what to do.'
          : creating && !/^[a-z0-9_-]{1,64}$/.test(slug)
            ? 'The name may use lowercase letters, digits, - and _.'
            : missingFields(fields, d.values).length
              ? `Fill in ${missingFields(fields, d.values)[0].label.toLowerCase()}.`
              : d.triage && !d.criteria.trim()
                ? 'Triage needs to know what counts as important.'
                : needsModel
                  ? choosesModel
                      ? 'Choose a model for triage.'
                      : 'Triage needs a model, and you may not choose models.'
                  : undefined
    const back = creating ? fromList : `/${agentId}/triggers/${encodeURIComponent(trigger.slug)}`
    const leave = () => navigate(back, { state: creating ? undefined : carry })

    const chooseKind = (id: string) => {
        const next = kindOf(kinds, id)
        set({ kind: id, values: fieldValues(next?.listing.fields ?? []), scope: next?.default_scope ?? null })
    }

    const submit = async () => {
        if (problem || busy) return
        setBusy(true)
        setError(undefined)
        const input = inputOf(d, kind, trigger?.settings ?? {})
        try {
            if (creating) {
                const made = await createTrigger(agentId, { ...input, slug, kind: d.kind })
                if (made.secret) holdSecret(agentId, slug, made.secret)
            } else {
                // Only what changed: a field sent unchanged is still checked as a change.
                const before = inputOf(start, kind, trigger.settings)
                const changed = Object.fromEntries(
                    Object.entries(input).filter(([k, v]) => JSON.stringify(v) !== JSON.stringify(before[k as keyof TriggerInput]))
                ) as TriggerInput
                await updateTrigger(agentId, trigger.slug, changed)
            }
            useTriggers.getState().changed(agentId)
            toast.success(creating ? 'Trigger created' : 'Trigger saved')
            navigate(`/${agentId}/triggers/${encodeURIComponent(slug)}`, { state: carry, replace: true })
        } catch (e) {
            setError(e instanceof ApiError || e instanceof Error ? e.message : 'Something went wrong')
        } finally {
            setBusy(false)
        }
    }

    const heading = (title: string) => <h2 className="border-t border-border pt-20 type-heading text-fg">{title}</h2>
    const destinationOptions = [
        { value: '', label: 'Only the inbox' },
        ...destinations,
        ...(d.destination && !destinations.some((o) => o.value === d.destination) ? [{ value: d.destination, label: 'The conversation it posts to now' }] : []),
    ]
    const summaries = ['0', '1', '5', '12', '24', ...(['0', '1', '5', '12', '24'].includes(start.summary) ? [] : [start.summary])]

    return (
        <PageFrame
            fill
            back={{ href: back, label: creating ? 'Triggers' : trigger.title, state: creating ? undefined : carry }}
            title={creating ? 'New trigger' : `Edit ${trigger.title}`}
            meta={slug ? `${slug}${creating ? '' : ' · the name is fixed once created'}` : 'Starts a run when something happens'}
            actions={
                <>
                    {!mobile && problem && <span className="type-small text-faint">{problem}</span>}
                    <Button variant="ghost" onClick={leave}>
                        Cancel
                    </Button>
                    <Button type="submit" form={FORM} variant="primary" loading={busy} disabled={!!problem}>
                        {creating ? 'Create trigger' : 'Save trigger'}
                    </Button>
                </>
            }
        >
            <form
                id={FORM}
                className="flex min-h-0 flex-1 flex-col overflow-y-auto px-16 py-20 sm:px-24 sm:py-24"
                onSubmit={(e) => {
                    e.preventDefault()
                    void submit()
                }}
            >
                <div className="flex w-full max-w-3xl flex-col gap-20 [&>*]:shrink-0">
                    {error && <Notice tone="failed" title="Couldn't save the trigger">{error}</Notice>}
                    {mobile && problem && <p className="type-small text-faint">{problem}</p>}
                    {creating && (
                        <fieldset className="flex flex-col gap-12">
                            <legend className="mb-8 type-ui-medium text-soft">Kind</legend>
                            {kinds.map((k) => (
                                <div key={k.id} className="flex flex-col gap-2">
                                    <Radio name="kind" label={k.listing.name} checked={d.kind === k.id} disabled={k.public && !publishes} onChange={() => chooseKind(k.id)} />
                                    <p className="pl-24 type-small text-muted">
                                        {k.listing.description}.{k.public && !publishes && ' You may not open public webhooks on this agent.'}
                                    </p>
                                </div>
                            ))}
                        </fieldset>
                    )}
                    <div className="grid grid-cols-1 gap-16 sm:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]">
                        <TextField label="Title" value={d.title} onChange={(e) => set({ title: e.target.value })} autoFocus={creating} />
                        <TextField label="Name" mono locked={!creating} value={creating ? d.slug : slug} placeholder={slugOf(d.title)} onChange={(e) => set({ slug: e.target.value })} />
                    </div>
                    <Textarea label="What to do when it fires" rows={4} value={d.instruction} onChange={(e) => set({ instruction: e.target.value })} />

                    {kind && (
                        <>
                            {heading(`${kind.listing.name} settings`)}
                            <KindFields key={kind.id} fields={fields} values={d.values} saved={trigger?.settings} onChange={(key, value) => setDraft((prev) => ({ ...prev, values: { ...prev.values, [key]: value } }))} />
                        </>
                    )}

                    {heading('Triage')}
                    <SwitchRow label="Judge each event first" helper="A model decides which events are worth a run. Without it, every event counts." checked={d.triage} onCheckedChange={(triage) => set({ triage })} />
                    {d.triage && (
                        <>
                            <Textarea label="What counts as important" rows={3} value={d.criteria} onChange={(e) => set({ criteria: e.target.value })} />
                            {choosesModel ? (
                                <ModelSelect compact provider={d.provider} model={d.model} onChange={(c) => set({ provider: c.provider, model: c.model })} />
                            ) : (
                                <p className="type-small text-muted">{d.model ? `Judged by ${d.provider} / ${d.model}. ` : ''}Choosing a model needs the model permission.</p>
                            )}
                            <TextField label="Triage calls per hour" mono inputMode="numeric" className="max-w-180" value={d.calls} onChange={(e) => set({ calls: e.target.value })} />
                        </>
                    )}

                    {heading('Results')}
                    <div className="grid grid-cols-1 gap-16 sm:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]">
                        <Select label="Also post to one of your conversations" value={d.destination} options={destinationOptions} onChange={(destination) => set({ destination })} />
                        <Select
                            label="Quiet summary"
                            value={d.summary}
                            options={summaries.map((h) => ({ value: h, label: h === '0' ? 'Off' : h === '1' ? 'Every hour' : `Every ${h} hours` }))}
                            onChange={(summary) => set({ summary })}
                        />
                    </div>

                    {heading('Limits')}
                    <SwitchRow label="Limit runs per hour" checked={d.limited} onCheckedChange={(limited) => set({ limited })} />
                    <div className="grid grid-cols-1 gap-16 sm:grid-cols-3">
                        <TextField label="Runs per hour" mono inputMode="numeric" disabled={!d.limited} value={d.limited ? d.limit : ''} onChange={(e) => set({ limit: e.target.value })} />
                        <TextField label="Cooldown" mono inputMode="numeric" suffix="s" value={d.cooldown} onChange={(e) => set({ cooldown: e.target.value })} />
                        <Select
                            label="While a run is going"
                            value={d.overlap}
                            options={[
                                { value: 'wait', label: 'Events wait' },
                                { value: 'drop', label: 'Events are dropped' },
                            ]}
                            onChange={(overlap) => set({ overlap })}
                        />
                    </div>

                    {heading('Tool access')}
                    <ScopeEditor scope={d.scope} onChange={(scope) => set({ scope })} />
                    <LimitWarnings scope={d.scope} limit={d.limited ? 1 : null} isPublic={kind?.public} />
                </div>
            </form>
        </PageFrame>
    )
}
