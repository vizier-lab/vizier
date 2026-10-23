import { ActionBar, AgentPreviewCard, Button, Card, Notice, Stepper, TextField, Textarea } from '@vizier/design'
import { useMemo, useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router'
import { ApiError } from '../api/client'
import { createAgent } from '../api/endpoints/agents'
import { emptyForm, problems, slugOf, toInput, type AgentForm } from '../lib/agentForm'
import { useMobile } from '../lib/useMobile'
import { ModelSelect } from '../screens/agents/ModelSelect'
import { ToolsFields } from '../screens/agents/ToolsFields'
import { PageFrame } from '../screens/shared/PageFrame'
import { TwoColumn } from '../screens/shared/TwoColumn'
import { useAgents } from '../stores/agents'
import { toast } from '../stores/toasts'

const STEPS = [
    { key: 'config', label: 'Config' },
    { key: 'prompt', label: 'System Prompt' },
    { key: 'tools', label: 'Tools' },
    { key: 'review', label: 'Review' },
] as const

type StepKey = (typeof STEPS)[number]['key']

function toolNames(f: AgentForm): string[] {
    const names: [boolean, string][] = [
        [f.fetch, 'Fetch'],
        [f.httpClient, 'HTTP'],
        [f.tts, 'Text-to-Speech'],
        [f.stt, 'Speech-to-Text'],
        [f.readImage, 'Read Image'],
        [f.imageGen, 'Image Generation'],
        [f.shell, 'Shell'],
    ]
    return names.filter(([on]) => on).map(([, n]) => n)
}

/** New agent: four steps, and nothing is saved until "Create agent". */
export default function AgentNew() {
    const mobile = useMobile()
    const navigate = useNavigate()
    const [params, setParams] = useSearchParams()
    const [form, setForm] = useState<AgentForm>(emptyForm)
    const [idEdited, setIdEdited] = useState(false)
    const [tried, setTried] = useState<Set<StepKey>>(new Set())
    const [busy, setBusy] = useState(false)
    const [refused, setRefused] = useState<string>()

    const asked = params.get('step')
    const index = Math.max(0, STEPS.findIndex((s) => s.key === asked))
    const step = STEPS[index].key

    const set = (patch: Partial<AgentForm>) => setForm((f) => ({ ...f, ...patch }))
    const errors = useMemo(() => problems(form, 'all'), [form])
    const stepErrors = (key: StepKey) => (key === 'review' ? {} : problems(form, key))
    const shown = tried.has(step) ? stepErrors(step) : {}

    const go = (to: number) =>
        setParams(
            (p) => {
                const next = new URLSearchParams(p)
                next.set('step', STEPS[to].key)
                return next
            },
            { replace: false }
        )

    const next = () => {
        if (Object.keys(stepErrors(step)).length) return setTried((t) => new Set(t).add(step))
        go(index + 1)
    }

    const create = async () => {
        if (Object.keys(errors).length) {
            // Back to the first step with something wrong.
            const bad = STEPS.find((s) => Object.keys(stepErrors(s.key)).length)
            if (bad) {
                setTried(new Set(STEPS.map((s) => s.key)))
                go(STEPS.indexOf(bad))
            }
            return
        }
        setBusy(true)
        setRefused(undefined)
        try {
            await createAgent(toInput(form))
            await useAgents.getState().refresh()
            toast.success(`${form.name} created`)
            navigate(`/${form.agentId}/chat`)
        } catch (e) {
            setRefused(e instanceof ApiError || e instanceof Error ? e.message : 'The agent could not be created.')
        } finally {
            setBusy(false)
        }
    }

    const tools = toolNames(form)
    const steps = STEPS.map((s, i) => ({
        label: s.label,
        state: i < index ? ('done' as const) : i === index ? ('current' as const) : ('upcoming' as const),
    }))

    const preview = (
        <div className="lg:sticky lg:top-0">
            <AgentPreviewCard
                agentId={form.agentId || 'new-agent'}
                name={form.name || 'Untitled'}
                description={form.description || undefined}
                facts={[
                    { label: 'Model', value: form.model ? `${form.provider}/${form.model}` : 'not chosen yet' },
                    { label: 'Dreams', value: form.dreamEnabled ? form.dreamSchedule : 'off' },
                    { label: 'Shell', value: form.shell ? (form.shellKind === 'docker' ? 'Docker' : 'Local') : 'none' },
                    { label: 'Tools', value: tools.length ? tools.join(', ') : 'always-on only' },
                ]}
                note="Its sigil is drawn from the agent ID, so it stays the same everywhere: here, in Discord and in Telegram."
            />
        </div>
    )

    return (
        <div className="flex min-h-0 flex-1 flex-col">
            <PageFrame
                title="New agent"
                meta={`Step ${index + 1} of ${STEPS.length} · nothing is saved until you create it`}
                actions={mobile ? undefined : <Stepper aria-label="New agent steps" steps={steps} />}
            >
                {mobile && (
                    <div className="relative overflow-x-auto">
                        <Stepper aria-label="New agent steps" steps={steps} look="compact" />
                    </div>
                )}
                <TwoColumn aside={preview}>
                    {step === 'config' && (
                        <>
                            <Card title="Identity">
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
                                        helper={shown.agentId ?? "It can't change after the agent is created."}
                                        onChange={(e) => {
                                            setIdEdited(true)
                                            set({ agentId: e.target.value })
                                        }}
                                    />
                                    <TextField label="Description" value={form.description} helper="Optional. Shown on Home and in the agent picker." onChange={(e) => set({ description: e.target.value })} />
                                </div>
                            </Card>
                            <Card title="Model" description="From the models.dev catalogue and your custom providers.">
                                <ModelSelect provider={form.provider} model={form.model} onChange={(c) => set({ provider: c.provider, model: c.model })} />
                                {(shown.provider || shown.model) && <p className="type-small text-failed-text">{shown.provider ?? shown.model}</p>}
                            </Card>
                        </>
                    )}

                    {step === 'prompt' && (
                        <Card title="System prompt" description="Optional. Who the agent is and how it should work. It can also edit this later in its Core.">
                            <Textarea aria-label="System prompt" mono rows={16} value={form.systemPrompt} onChange={(e) => set({ systemPrompt: e.target.value })} />
                        </Card>
                    )}

                    {step === 'tools' && <ToolsFields form={form} errors={shown} set={set} />}

                    {step === 'review' && (
                        <Card title="Ready to create" description="Check the preview on the right. You can change all of this later on Config.">
                            {refused && (
                                <Notice tone="failed" title="The agent could not be created">
                                    {refused}
                                </Notice>
                            )}
                        </Card>
                    )}
                </TwoColumn>
            </PageFrame>
            <ActionBar
                position="bottom"
                start={
                    index > 0 ? (
                        <Button icon="chevron-left" onClick={() => go(index - 1)}>
                            Back
                        </Button>
                    ) : (
                        <Button onClick={() => navigate(-1)}>Cancel</Button>
                    )
                }
                end={
                    step === 'review' ? (
                        <Button variant="primary" icon="plus" disabled={busy} onClick={create}>
                            {busy ? 'Creating…' : 'Create agent'}
                        </Button>
                    ) : (
                        <Button variant="primary" icon="chevron-right" onClick={next}>
                            {index === STEPS.length - 2 ? 'Continue to review' : 'Continue'}
                        </Button>
                    )
                }
            />
        </div>
    )
}
