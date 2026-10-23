import { Button, Card, Dialog, EmptyState, Notice, Select, ShareList, SigilPicker, Skeleton, Tab, TabList, TextField, Textarea, ToggleSection, UnsavedIndicator, ImageCropper } from '@vizier/design'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate, useParams, useSearchParams } from 'react-router'
import { ApiError } from '../api/client'
import { deleteAgent, getAgent, getSharing, updateAgent, updateSharing, type AgentDetail } from '../api/endpoints/agents'
import { listUsers, type UserSummary } from '../api/endpoints/auth'
import { EMBEDDING_PROVIDERS, fromDetail, problems, toInput, type AgentForm } from '../lib/agentForm'
import { uploadFile } from '../lib/upload'
import { ModelSelect } from '../screens/agents/ModelSelect'
import { ToolsFields } from '../screens/agents/ToolsFields'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { PageFrame } from '../screens/shared/PageFrame'
import { TwoColumn } from '../screens/shared/TwoColumn'
import { useAgents } from '../stores/agents'
import { toast } from '../stores/toasts'

const TABS = [
    { key: 'config', label: 'Config' },
    { key: 'prompt', label: 'System Prompt' },
    { key: 'tools', label: 'Tools' },
    { key: 'sharing', label: 'Sharing' },
    { key: 'danger', label: 'Danger Zone' },
] as const

type TabKey = (typeof TABS)[number]['key']

const same = (a: AgentForm, b: AgentForm) => JSON.stringify(a) === JSON.stringify(b)

export default function AgentConfig() {
    const { agentId = '' } = useParams()
    const navigate = useNavigate()
    const [params, setParams] = useSearchParams()
    const tab = (TABS.find((t) => t.key === params.get('tab'))?.key ?? 'config') as TabKey

    const [detail, setDetail] = useState<AgentDetail>()
    const [saved, setSaved] = useState<AgentForm>()
    const [form, setForm] = useState<AgentForm>()
    const [error, setError] = useState<string>()
    const [saveError, setSaveError] = useState<string>()
    const [confirming, setConfirming] = useState(false)
    const [saving, setSaving] = useState(false)
    const [picture, setPicture] = useState<File>()
    const [deleting, setDeleting] = useState(false)

    const load = useCallback(() => {
        setDetail(undefined)
        setForm(undefined)
        getAgent(agentId).then(
            (d) => {
                const f = fromDetail(d)
                setDetail(d)
                setSaved(f)
                setForm(f)
            },
            (e) => setError(e instanceof Error ? e.message : 'The agent could not be loaded.')
        )
    }, [agentId])
    useEffect(load, [load])

    const set = (patch: Partial<AgentForm>) => setForm((f) => (f ? { ...f, ...patch } : f))
    const dirty = !!form && !!saved && !same(form, saved)
    const errors = useMemo(() => (form ? problems(form, 'all', true) : {}), [form])

    // Leaving with unsaved changes asks the browser to confirm.
    const dirtyRef = useRef(dirty)
    dirtyRef.current = dirty
    useEffect(() => {
        const warn = (e: BeforeUnloadEvent) => dirtyRef.current && e.preventDefault()
        window.addEventListener('beforeunload', warn)
        return () => window.removeEventListener('beforeunload', warn)
    }, [])

    const save = async () => {
        if (!form || !detail) return
        setSaving(true)
        setSaveError(undefined)
        try {
            const result = await updateAgent(toInput(form, detail))
            setConfirming(false)
            toast.success('Saved', result?.warnings?.[0]?.message)
            await useAgents.getState().refresh()
            load()
        } catch (e) {
            setConfirming(false)
            // A config that can't start is refused and the agent keeps its previous one.
            setSaveError(e instanceof ApiError || e instanceof Error ? e.message : 'The change was refused.')
        } finally {
            setSaving(false)
        }
    }

    const choose = (key: TabKey) =>
        setParams(
            (p) => {
                const next = new URLSearchParams(p)
                next.set('tab', key)
                return next
            },
            { replace: true }
        )

    if (error) return <PageFrame title="Config"><EmptyState kind="error" title="Config" message={error} /></PageFrame>
    if (!form || !detail) {
        return (
            <PageFrame title="Config" width="narrow">
                <Skeleton className="h-40" />
                <Skeleton className="h-40" />
            </PageFrame>
        )
    }

    const needsSave = Object.keys(errors).length === 0

    const identity = (
        <Card title="Identity">
            <div className="flex gap-24">
                <SigilPicker agentId={form.agentId} imageSrc={form.avatarUrl || undefined} onUpload={setPicture} onReset={() => set({ avatarUrl: '' })} />
                <div className="flex min-w-0 flex-1 flex-col gap-16">
                    <div className="grid grid-cols-1 gap-16 sm:grid-cols-2">
                        <TextField label="Name" value={form.name} invalid={!!errors.name} helper={errors.name} onChange={(e) => set({ name: e.target.value })} />
                        <TextField label="Agent ID" mono locked value={form.agentId} readOnly />
                    </div>
                    <TextField label="Description" value={form.description} onChange={(e) => set({ description: e.target.value })} />
                </div>
            </div>
        </Card>
    )

    const model = (
        <Card title="Model" description="From the models.dev catalogue and your custom providers.">
            <div className="flex flex-col gap-16">
                <ModelSelect provider={form.provider} model={form.model} onChange={(c) => set({ provider: c.provider, model: c.model })} />
                <div className="grid grid-cols-1 gap-16 sm:grid-cols-3">
                    <TextField label="Context window" mono suffix="tokens" inputMode="numeric" value={form.contextWindow} invalid={!!errors.contextWindow} helper={errors.contextWindow ?? 'Empty uses the model’s own.'} onChange={(e) => set({ contextWindow: e.target.value })} />
                    <TextField label="Max output" mono suffix="tokens" inputMode="numeric" value={form.maxTokens} invalid={!!errors.maxTokens} helper={errors.maxTokens ?? 'Empty uses the model’s own.'} onChange={(e) => set({ maxTokens: e.target.value })} />
                    <TextField label="Thinking depth" mono suffix="steps" inputMode="numeric" value={form.thinkingDepth} helper="Most tool rounds in one turn." onChange={(e) => set({ thinkingDepth: e.target.value })} />
                </div>
            </div>
        </Card>
    )

    const behaviour = (
        <Card title="Behaviour">
            <div className="flex flex-col gap-16">
                <div className="grid grid-cols-1 gap-16 sm:grid-cols-3">
                    <TextField label="Prompt timeout" mono value={form.promptTimeout} helper="A turn stops after this long." onChange={(e) => set({ promptTimeout: e.target.value })} />
                    <TextField label="Tool timeout" mono value={form.toolsTimeout} helper="One tool call." onChange={(e) => set({ toolsTimeout: e.target.value })} />
                    <TextField label="Checkpoint threshold" mono suffix="%" inputMode="numeric" value={form.checkpointPercent} helper="Hand over to a fresh context at this share of the window." onChange={(e) => set({ checkpointPercent: e.target.value })} />
                </div>
                <ToggleSection title="Dreams" description="Reflect and tidy memory on a schedule." checked={form.dreamEnabled} onCheckedChange={(dreamEnabled) => set({ dreamEnabled })}>
                    <div className="flex flex-col gap-16">
                        <TextField label="Dream schedule" mono value={form.dreamSchedule} helper="A cron expression, e.g. 0 3 * * *." onChange={(e) => set({ dreamSchedule: e.target.value })} />
                        <ModelSelect providerLabel="Dream provider" modelLabel="Dream model" provider={form.dreamProvider || form.provider} model={form.dreamModel || form.model} onChange={(c) => set({ dreamProvider: c.provider, dreamModel: c.model })} />
                    </div>
                </ToggleSection>
                <TextField label="Silent read chance" mono suffix="%" inputMode="numeric" value={form.silentReadPercent} helper="How often it joins a group conversation on Discord and Telegram unprompted." onChange={(e) => set({ silentReadPercent: e.target.value })} />
            </div>
        </Card>
    )

    const memoryIndex = (
        <Card title="Memory index" description="Embeddings for passage search.">
            <div className="flex flex-col gap-16">
                <Select label="Indexer" options={[{ value: 'sqlite', label: 'SQLite (vector store)' }]} value="sqlite" onChange={() => {}} disabled />
                <Select label="Embedding provider" options={EMBEDDING_PROVIDERS.map((p) => ({ value: p, label: p }))} value={form.embeddingProvider} onChange={(embeddingProvider) => set({ embeddingProvider })} />
                <TextField label="Embedding model" mono value={form.embeddingModel} invalid={!!errors.embeddingModel} helper={errors.embeddingModel ?? (form.embeddingProvider === 'local' ? 'Empty uses the built-in model.' : 'Changing it re-indexes memory when the agent starts.')} onChange={(e) => set({ embeddingModel: e.target.value })} />
            </div>
        </Card>
    )

    const sharing = (
        <Card title="Sharing" description={`People who can chat with ${form.name}.`}>
            <Sharing agentId={agentId} />
        </Card>
    )

    const danger = (
        <Card title="Danger zone" tone="danger">
            <div className="flex flex-col gap-12">
                <p className="type-body text-muted">Delete {form.name} with its memory, tasks, sessions and integrations. Usage history is removed too. This can’t be undone.</p>
                <div>
                    <Button variant="destructive" icon="trash" onClick={() => setDeleting(true)}>
                        Delete agent
                    </Button>
                </div>
            </div>
        </Card>
    )

    return (
        <PageFrame
            title="Config"
            meta={`${form.name} · @${form.agentId}`}
            actions={
                <>
                    <span className="hidden type-small text-faint xl:inline">Saving rebuilds {form.name} and stops its running turns.</span>
                    {dirty && <UnsavedIndicator>Unsaved changes</UnsavedIndicator>}
                    {dirty && (
                        <Button disabled={saving} onClick={() => (setForm(saved), setSaveError(undefined))}>
                            Discard
                        </Button>
                    )}
                    <Button variant="primary" icon="save" disabled={!dirty || !needsSave || saving} onClick={() => setConfirming(true)}>
                        Save changes
                    </Button>
                </>
            }
        >
            {saveError && (
                <Notice tone="failed" title="The change was refused. The agent keeps running on its previous settings.">
                    {saveError}
                </Notice>
            )}

            <TabList aria-label="Agent settings">
                {TABS.map((t) => (
                    <Tab key={t.key} selected={t.key === tab} danger={t.key === 'danger'} onClick={() => choose(t.key)}>
                        {t.label}
                    </Tab>
                ))}
            </TabList>

            <div role="tabpanel" aria-label={TABS.find((t) => t.key === tab)?.label}>
                {tab === 'config' && (
                    <TwoColumn
                        aside={
                            <>
                                {memoryIndex}
                                {sharing}
                                {danger}
                            </>
                        }
                    >
                        {identity}
                        {model}
                        {behaviour}
                    </TwoColumn>
                )}
                {tab === 'prompt' && (
                    <Card title="System prompt" description="Who the agent is and how it should work.">
                        <Textarea aria-label="System prompt" mono rows={22} value={form.systemPrompt} onChange={(e) => set({ systemPrompt: e.target.value })} />
                    </Card>
                )}
                {tab === 'tools' && <ToolsFields form={form} errors={errors} set={set} />}
                {tab === 'sharing' && sharing}
                {tab === 'danger' && danger}
            </div>

            <Dialog
                open={confirming}
                title={`Save changes to ${form.name}?`}
                message={`Saving rebuilds ${form.name} and stops its running turns. Waiting messages run after it starts again.`}
                confirmLabel={saving ? 'Saving…' : 'Save'}
                onConfirm={save}
                onCancel={() => setConfirming(false)}
            />

            <ConfirmDialog
                open={deleting}
                title={`Delete ${form.name}?`}
                message="These are removed with it, and this can’t be undone:"
                removes={['Its memory bundles and documents', 'Its tasks, skills and Core history', 'Its conversations and files', 'Its channel, addon and MCP connections']}
                confirmWord={agentId}
                confirmLabel={`Delete ${form.name}`}
                onConfirm={async () => {
                    try {
                        await deleteAgent(agentId)
                        await useAgents.getState().refresh()
                        toast.success(`${form.name} deleted`)
                        navigate('/')
                    } catch (e) {
                        setDeleting(false)
                        toast.error('Couldn’t delete the agent', e instanceof Error ? e.message : undefined)
                    }
                }}
                onCancel={() => setDeleting(false)}
            />

            {picture && (
                <ImageCropper
                    file={picture}
                    onCancel={() => setPicture(undefined)}
                    onCrop={async (blob) => {
                        try {
                            const up = await uploadFile(new File([blob], 'avatar.png', { type: 'image/png' }))
                            const local = typeof up.content === 'object' && 'local' in up.content ? up.content.local : ''
                            set({ avatarUrl: local })
                        } catch (e) {
                            toast.error('Couldn’t upload the picture', e instanceof Error ? e.message : undefined)
                        }
                        setPicture(undefined)
                    }}
                />
            )}
        </PageFrame>
    )
}

/** Who else can use the agent. Saved at once: sharing doesn't rebuild the agent. */
function Sharing({ agentId }: { agentId: string }) {
    const [shared, setShared] = useState<string[]>()
    const [users, setUsers] = useState<UserSummary[]>([])
    const [problem, setProblem] = useState<string>()

    useEffect(() => {
        getSharing(agentId).then((s) => setShared(s.shared_to), (e) => setProblem(e instanceof Error ? e.message : 'Sharing could not be loaded.'))
        listUsers().then(setUsers, () => setUsers([]))
    }, [agentId])

    const nameOf = (id: string) => users.find((u) => u.user_id === id)?.username ?? id
    const change = async (change: { add?: string[]; remove?: string[] }) => {
        try {
            setShared((await updateSharing(agentId, change)).shared_to)
        } catch (e) {
            toast.error('Couldn’t change sharing', e instanceof Error ? e.message : undefined)
        }
    }

    if (problem) return <Notice tone="failed">{problem}</Notice>
    if (!shared) return <Skeleton className="h-40" />
    return (
        <ShareList
            people={shared.map((id) => ({ name: nameOf(id), permission: 'Can use' }))}
            onRemove={(name) => {
                const id = users.find((u) => u.username === name)?.user_id ?? name
                void change({ remove: [id] })
            }}
            onShare={(username) => {
                const user = users.find((u) => u.username.toLowerCase() === username.trim().toLowerCase())
                if (!user) return toast.error(`No one is called ${username}`)
                void change({ add: [user.user_id] })
            }}
        />
    )
}
