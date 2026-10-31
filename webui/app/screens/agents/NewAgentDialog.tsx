import {
    AddonTile,
    AgentPreviewCard,
    Button,
    Card,
    Checkbox,
    IconButton,
    KeyValueList,
    Modal,
    Notice,
    Skeleton,
    Stepper,
    TextField,
} from '@vizier/design'
import { useEffect, useRef, useState } from 'react'
import { useSearchParams } from 'react-router'
import { ApiError } from '../../api/client'
import { createAgent } from '../../api/endpoints/agents'
import { createIntegration, integrationKinds, setGrant, type Kind } from '../../api/endpoints/integrations'
import { emptyForm, problems, slugOf, toInput, type AgentForm } from '../../lib/agentForm'
import { PERMISSIONS, useCan } from '../../lib/access'
import { useMobile } from '../../lib/useMobile'
import { AddonDialog } from '../integrations/AddonForm'
import { noGrants, SelfManagementDraft, type GrantDraft } from '../integrations/SelfManagementCard'
import { ModelsFields } from './ModelsFields'
import { useAgents } from '../../stores/agents'
import { toast } from '../../stores/toasts'

const STEPS = [
    { key: 'identity', label: 'Identity' },
    { key: 'models', label: 'Models' },
    { key: 'addons', label: 'Addons' },
    { key: 'prompt', label: 'Prompt' },
    { key: 'review', label: 'Review' },
] as const

type StepKey = (typeof STEPS)[number]['key']

/** Steps whose validation is not the agent form's own. */
const STEP_PROBLEMS: Record<StepKey, 'config' | 'models' | 'addons' | 'prompt' | 'all'> = {
    identity: 'config',
    models: 'models',
    addons: 'addons',
    prompt: 'prompt',
    review: 'all',
}

/** The addons chosen for the first run, with the settings they were given. */
type Chosen = Record<string, Record<string, unknown>>

/** The `new=1` query flag that opens creation, on any page. */
export const NEW_AGENT_PARAM = 'new'

/** Add the flag to a path without dropping the page's own query. */
export function withNewAgent(href: string): string {
    const url = new URL(href, 'http://vizier.local')
    url.searchParams.set(NEW_AGENT_PARAM, '1')
    return `${url.pathname}?${url.searchParams.toString()}`
}

/** True when the URL asks for the creation dialog. */
export const newAgentOpen = (params: URLSearchParams) => params.get(NEW_AGENT_PARAM) === '1'

/** New agent: five steps in a modal, and nothing is saved until "Create agent". */
export function NewAgentDialog() {
    const [params, setParams] = useSearchParams()
    const open = newAgentOpen(params)
    const mobile = useMobile()
    const canShell = useCan(PERMISSIONS.shellConfig)

    const [form, setForm] = useState<AgentForm>(emptyForm)
    const [idEdited, setIdEdited] = useState(false)
    const [index, setIndex] = useState(0)
    const [tried, setTried] = useState<Set<StepKey>>(new Set())
    const [refused, setRefused] = useState<string>()
    const [grants, setGrants] = useState<GrantDraft>(noGrants)
    const [kinds, setKinds] = useState<Kind[]>()
    const [kindsFailed, setKindsFailed] = useState(false)
    const [chosen, setChosen] = useState<Chosen>({})
    const [configuring, setConfiguring] = useState<Kind | null>(null)
    // A refused creation reopens the dialog where it was: the form is kept rather than reset.
    const kept = useRef(false)

    // Each opening starts clean; the fields never carry over from the last agent. A `step` in the
    // URL opens a particular step (a link can point at the addons, say).
    useEffect(() => {
        if (!open) return
        if (kept.current) {
            kept.current = false
            return
        }
        const asked = STEPS.findIndex((s) => s.key === params.get('step'))
        setForm(emptyForm())
        setIdEdited(false)
        setIndex(asked >= 0 ? asked : 0)
        setTried(new Set())
        setRefused(undefined)
        setGrants(noGrants)
        setChosen({})
        setConfiguring(null)
    }, [open])

    useEffect(() => {
        if (!open || kinds) return
        integrationKinds('addon').then(setKinds, () => setKindsFailed(true))
    }, [open, kinds])

    const step = STEPS[index].key
    const set = (patch: Partial<AgentForm>) => setForm((f) => ({ ...f, ...patch }))
    const stepErrors = (key: StepKey) => problems(form, STEP_PROBLEMS[key])
    const shown = tried.has(step) ? stepErrors(step) : {}
    const allowedAddon = (kind: string) => kind !== 'shell' || canShell

    const close = () => {
        const next = new URLSearchParams(params)
        next.delete(NEW_AGENT_PARAM)
        setParams(next, { replace: true })
    }

    const next = () => {
        if (Object.keys(stepErrors(step)).length) return setTried((t) => new Set(t).add(step))
        setIndex((i) => Math.min(STEPS.length - 1, i + 1))
    }

    const back = () => setIndex((i) => Math.max(0, i - 1))

    const chooseAddon = (kind: Kind) => {
        if (kind.fields.length > 0) return setConfiguring(kind)
        setChosen((c) => ({ ...c, [kind.id]: {} }))
    }

    const create = () => {
        const bad = STEPS.find((s) => Object.keys(stepErrors(s.key)).length)
        if (bad) {
            setTried(new Set(STEPS.map((s) => s.key)))
            setIndex(STEPS.indexOf(bad))
            return
        }
        // The dialog closes at once: creating an agent can wait on a model download for minutes, and
        // the person should be able to keep working. The new agent's tile appears in the rail, busy,
        // until the server has it; a refusal reopens this form exactly as it was.
        const input = toInput(form)
        const { agentId, name } = form
        const granted = (Object.keys(grants) as (keyof GrantDraft)[]).filter((family) => grants[family])
        const installing = (kinds ?? []).filter((k) => chosen[k.id] !== undefined)
        setRefused(undefined)
        close()
        useAgents.getState().startCreating({ agentId, name })
        void (async () => {
            try {
                await createAgent(input)
                // Grants and addons live apart from the agent's config, so they are set once it exists.
                for (const family of granted) {
                    try {
                        await setGrant(agentId, family, true)
                    } catch (e) {
                        toast.error(`Couldn’t switch on self-management for ${family}`, e instanceof Error ? e.message : undefined)
                    }
                }
                for (const kind of installing) {
                    try {
                        await createIntegration(agentId, { kind: kind.id, name: kind.name, settings: chosen[kind.id] ?? {} })
                    } catch (e) {
                        toast.error(`Couldn’t add ${kind.name}`, e instanceof Error ? e.message : undefined)
                    }
                }
                await useAgents.getState().refresh()
                toast.success(`${name} created`)
            } catch (e) {
                kept.current = true
                setRefused(e instanceof ApiError || e instanceof Error ? e.message : 'The agent could not be created.')
                setIndex(STEPS.length - 1)
                const reopened = new URLSearchParams(params)
                reopened.set(NEW_AGENT_PARAM, '1')
                setParams(reopened, { replace: true })
            } finally {
                useAgents.getState().finishCreating(agentId)
            }
        })()
    }

    const steps = STEPS.map((s, i) => ({
        label: s.label,
        state: i < index ? ('done' as const) : i === index ? ('current' as const) : ('upcoming' as const),
    }))
    const chosenKinds = (kinds ?? []).filter((k) => chosen[k.id] !== undefined)

    const preview = (
        <div className="hidden lg:sticky lg:top-0 lg:block">
            <AgentPreviewCard
                agentId={form.agentId || 'new-agent'}
                name={form.name || 'Untitled'}
                description={form.description || undefined}
                facts={[
                    { label: 'Model', value: form.model ? `${form.provider}/${form.model}` : 'not chosen yet' },
                    { label: 'Voice', value: form.tts || form.stt ? [form.tts && 'speaks', form.stt && 'listens'].filter(Boolean).join(' and ') : 'off' },
                    { label: 'Pictures', value: form.readImage || form.imageGen ? [form.readImage && 'reads', form.imageGen && 'draws'].filter(Boolean).join(' and ') : 'off' },
                    { label: 'Addons', value: chosenKinds.length ? chosenKinds.map((k) => k.name).join(', ') : 'none yet' },
                    { label: 'Dreams', value: form.dreamEnabled ? form.dreamSchedule : 'off' },
                ]}
                note="Its sigil is drawn from the agent ID, so it stays the same everywhere: here, Discord and Telegram."
            />
        </div>
    )

    return (
        <Modal open={open} onClose={close} label="New agent">
            <div className="flex max-h-[88vh] w-[920px] max-w-[94vw] flex-col overflow-hidden rounded-xl border border-border-strong bg-surface shadow-dialog">
                <header className="flex items-center gap-16 border-b border-border px-24 py-16">
                    <div className="flex min-w-0 flex-1 flex-col gap-2">
                        <h2 className="type-heading text-fg">New agent</h2>
                        <p className="type-small text-faint">Step {index + 1} of {STEPS.length}</p>
                    </div>
                    {!mobile && <Stepper aria-label="New agent steps" steps={steps} />}
                    <IconButton variant="ghost" icon="close" aria-label="Close" onClick={close} />
                </header>
                {mobile && (
                    <div className="overflow-x-auto border-b border-border px-20 py-10">
                        <Stepper aria-label="New agent steps" steps={steps} look="compact" />
                    </div>
                )}

                <div className="min-h-0 flex-1 overflow-y-auto px-24 py-20">
                    <div className="grid grid-cols-1 items-start gap-24 lg:min-h-[460px] lg:grid-cols-[minmax(0,1fr)_268px]">
                        <div className="flex min-w-0 flex-col gap-16">
                            {step === 'identity' && (
                                <Card title="Identity" description="What people call it, and the id its sigil and links are built from.">
                                    <div className="flex flex-col gap-16">
                                        <TextField
                                            label="Name"
                                            value={form.name}
                                            invalid={!!shown.name}
                                            helper={shown.name ?? 'What people call it.'}
                                            autoFocus
                                            onChange={(e) => {
                                                const name = e.target.value
                                                set(idEdited ? { name } : { name, agentId: slugOf(name) })
                                            }}
                                        />
                                        <TextField
                                            label="Agent ID"
                                            mono
                                            value={form.agentId}
                                            invalid={!!shown.agentId}
                                            helper={shown.agentId ?? 'It can’t change after the agent is created.'}
                                            onChange={(e) => {
                                                setIdEdited(true)
                                                set({ agentId: e.target.value })
                                            }}
                                        />
                                        <TextField label="Description" value={form.description} helper="Optional. Shown on Home and in the agent picker." onChange={(e) => set({ description: e.target.value })} />
                                    </div>
                                </Card>
                            )}

                            {step === 'models' && <ModelsFields form={form} errors={shown} set={set} />}

                            {step === 'addons' && (
                                <>
                                    <div className="flex flex-col gap-4">
                                        <h3 className="type-ui-strong text-fg">First addons</h3>
                                        <p className="type-ui-para text-muted">
                                            Add tools now, or skip and choose them later. Memory, Core, tasks, skills and code mode are always on.
                                        </p>
                                    </div>
                                    {kindsFailed ? (
                                        <Notice tone="failed" title="The addon catalogue could not be loaded">
                                            You can add addons from the agent’s Addons page after creating it.
                                        </Notice>
                                    ) : !kinds ? (
                                        <div className="grid grid-cols-1 gap-12 sm:grid-cols-2">
                                            <Skeleton className="h-156" />
                                            <Skeleton className="h-156" />
                                        </div>
                                    ) : (
                                        <div className="grid grid-cols-1 gap-12 sm:grid-cols-2">
                                            {kinds.map((kind) => {
                                                const picked = chosen[kind.id] !== undefined
                                                const blocked = !allowedAddon(kind.id)
                                                return (
                                                    <AddonTile
                                                        key={kind.id}
                                                        kind={kind.id}
                                                        name={kind.name}
                                                        description={kind.description}
                                                        selected={picked}
                                                    >
                                                        <span className="type-small text-faint">
                                                            {blocked ? 'Needs the shell permission' : picked ? 'Added on creation' : kind.fields.some((f) => f.required) ? 'Needs setup' : 'Ready to add'}
                                                        </span>
                                                        <div className="flex items-center gap-6">
                                                            {picked && kind.fields.length > 0 && (
                                                                <Button size="sm" variant="ghost" icon="settings" onClick={() => setConfiguring(kind)}>
                                                                    Settings
                                                                </Button>
                                                            )}
                                                            <Checkbox
                                                                label="Choose"
                                                                aria-label={`${picked ? 'Remove' : 'Add'} ${kind.name}`}
                                                                checked={picked}
                                                                disabled={blocked}
                                                                onChange={(e) =>
                                                                    setChosen((c) => {
                                                                        const next = { ...c }
                                                                        if (e.target.checked) void chooseAddon(kind)
                                                                        else delete next[kind.id]
                                                                        return next
                                                                    })
                                                                }
                                                            />
                                                        </div>
                                                    </AddonTile>
                                                )
                                            })}
                                        </div>
                                    )}
                                    <SelfManagementDraft value={grants} onChange={setGrants} />
                                </>
                            )}

                            {step === 'prompt' && (
                                <Card title="System prompt" description="Optional. Who the agent is and how it should work. It can also edit this later in its Core.">
                                    <textarea
                                        aria-label="System prompt"
                                        value={form.systemPrompt}
                                        onChange={(e) => set({ systemPrompt: e.target.value })}
                                        rows={10}
                                        placeholder="You are a composed, capable steward…"
                                        className="w-full resize-y rounded-md border border-border-strong bg-ground px-12 py-10 type-body text-fg placeholder:text-faint focus-visible:border-live focus-visible:outline-none"
                                    />
                                </Card>
                            )}

                            {step === 'review' && (
                                <Card title="Ready to create" description="Check the preview beside this. Everything can be changed later, including addons and the agent’s model.">
                                    <div className="flex flex-col gap-16">
                                        {refused && (
                                            <Notice tone="failed" title="The agent could not be created">
                                                {refused}
                                            </Notice>
                                        )}
                                        <KeyValueList
                                            items={[
                                                { label: 'Name', value: form.name || '—' },
                                                { label: 'Agent ID', value: <span className="type-code">{form.agentId || '—'}</span> },
                                                { label: 'Model', value: form.model ? `${form.provider}/${form.model}` : '—' },
                                                { label: 'Addons', value: chosenKinds.length ? chosenKinds.map((k) => k.name).join(', ') : 'none — always-on tools only' },
                                                { label: 'Prompt', value: form.systemPrompt.trim() ? `${form.systemPrompt.trim().split(/\s+/).length} words` : 'none yet' },
                                            ]}
                                        />
                                    </div>
                                </Card>
                            )}
                        </div>
                        {preview}
                    </div>
                </div>

                <footer className="flex items-center justify-between gap-12 border-t border-border bg-sidebar px-24 py-14">
                    <div className="flex items-center gap-10">
                        <Button onClick={index === 0 ? close : back}>{index === 0 ? 'Cancel' : 'Back'}</Button>
                        <span className="hidden type-small text-faint sm:inline">Nothing is saved until you create it.</span>
                    </div>
                    <div className="flex items-center gap-10">
                        {(step === 'addons' || step === 'prompt') && (
                            <Button variant="ghost" onClick={() => setIndex((i) => Math.min(STEPS.length - 1, i + 1))}>
                                Skip
                            </Button>
                        )}
                        {step === 'review' ? (
                            <Button variant="primary" icon="plus" onClick={create}>
                                Create agent
                            </Button>
                        ) : (
                            <Button variant="primary" icon="chevron-right" onClick={next}>
                                Continue
                            </Button>
                        )}
                    </div>
                </footer>
            </div>

            <AddonDialog
                kind={configuring}
                mode="install"
                onClose={() => setConfiguring(null)}
                onSave={async (kind, settings) => {
                    setChosen((c) => ({ ...c, [kind.id]: settings }))
                    setConfiguring(null)
                }}
            />
        </Modal>
    )
}
