import { ActionBar, Breadcrumb, Button, Dialog, KeywordInput, MarkdownEditor, Notice, TextField, UnsavedIndicator } from '@vizier/design'
import { useEffect, useState } from 'react'
import { useNavigate, useParams, useSearchParams } from 'react-router'
import { ApiError } from '../api/client'
import { createSkill, getSkill, updateSkill, type Skill } from '../api/endpoints/skills'
import { absoluteTime } from '../lib/format'
import { slugOf } from '../lib/tasks'
import { clearDraft, pendingDraft, saveDraft } from '../stores/drafts'
import { toast } from '../stores/toasts'

type Form = { description: string; keywords: string[]; content: string }

const BLANK: Form = {
    description: '',
    keywords: [],
    content: '# New skill\n\nWhat the agent should do when this skill is used.\n',
}

const same = (a: Form, b: Form) => a.description === b.description && a.content === b.content && a.keywords.join('\u0000') === b.keywords.join('\u0000')

export default function SkillEdit() {
    const { agentId = '', slug = '' } = useParams()
    const [params] = useSearchParams()
    const navigate = useNavigate()
    const scope = params.get('scope') === 'global' ? 'global' : 'agent'
    const creating = slug === 'new'
    const draftKey = `skill:${agentId}:${creating ? '(new)' : slug}`

    const [loaded, setLoaded] = useState<Skill | null>(null)
    const [name, setName] = useState('')
    const [form, setForm] = useState<Form>(BLANK)
    const [base, setBase] = useState<Form>(BLANK)
    const [saving, setSaving] = useState(false)
    const [conflict, setConflict] = useState<Skill | null>(null)
    const [error, setError] = useState<string>()
    const [draft, setDraft] = useState<ReturnType<typeof pendingDraft>>(null)

    useEffect(() => {
        if (creating) {
            setBase(BLANK)
            setDraft(pendingDraft(draftKey, BLANK.content))
            return
        }
        getSkill(scope, agentId, slug)
            .then((s) => {
                setLoaded(s)
                const f = { description: s.description, keywords: s.keywords, content: s.content }
                setForm(f)
                setBase(f)
                setDraft(pendingDraft(draftKey, s.content))
            })
            .catch((e) => toast.error("Couldn't load the skill", e.message))
    }, [agentId, slug, scope, creating, draftKey])

    const dirty = !same(form, base) || (creating && name !== '')
    const change = (patch: Partial<Form>) => {
        const next = { ...form, ...patch }
        setForm(next)
        if ('content' in patch) saveDraft(draftKey, next.content, base.content)
    }

    const finalName = creating ? slugOf(name) : slug
    const canSave = dirty && form.description.trim() !== '' && form.content.trim() !== '' && (!creating || finalName !== '')

    const save = async (overrideHash?: string) => {
        if (!canSave || saving) return
        setSaving(true)
        setError(undefined)
        try {
            if (creating) {
                await createSkill(scope, agentId, { name: finalName, description: form.description.trim(), content: form.content, keywords: form.keywords })
                clearDraft(draftKey)
                toast.success('Skill created')
                navigate(`/${agentId}/skills?skill=${encodeURIComponent(finalName)}`)
                return
            }
            const saved = await updateSkill(scope, agentId, slug, {
                description: form.description.trim(),
                content: form.content,
                keywords: form.keywords,
                base_hash: overrideHash ?? loaded?.base_hash,
            })
            clearDraft(draftKey)
            setLoaded(saved)
            setBase(form)
            toast.success('Skill saved')
        } catch (e) {
            if (e instanceof ApiError && e.status === 409) setConflict(e.data as Skill)
            else setError(e instanceof Error ? e.message : 'Something went wrong')
        } finally {
            setSaving(false)
        }
    }

    const discard = () => {
        clearDraft(draftKey)
        setForm(base)
        if (creating) setName('')
    }

    return (
        <div className="flex min-h-0 flex-1 flex-col">
            <ActionBar
                position="top"
                start={
                    <Breadcrumb
                        back={{ label: 'Skills', href: `/${agentId}/skills` }}
                        segments={[{ label: 'skills', href: `/${agentId}/skills` }, { label: creating ? 'new' : slug }]}
                    />
                }
                end={
                    <>
                        {dirty && <UnsavedIndicator>Unsaved · draft kept in this browser</UnsavedIndicator>}
                        <Button variant="ghost" disabled={!dirty} onClick={discard}>
                            Discard
                        </Button>
                        <Button variant="primary" loading={saving} disabled={!canSave} shortcut="Ctrl S" onClick={() => void save()}>
                            Save
                        </Button>
                    </>
                }
            />
            <div className="flex min-h-0 flex-1 flex-col gap-16 overflow-y-auto px-32 py-24">
                {error && <Notice tone="failed" title="Couldn't save the skill">{error}</Notice>}
                {draft && !dirty && (
                    <div className="flex items-center gap-12 rounded-lg border border-border-strong bg-surface px-16 py-12 type-small text-muted">
                        <span className="flex-1">You have an unsaved draft from {absoluteTime(draft.at)}.{draft.stale ? ' The skill has changed since.' : ''}</span>
                        <Button size="sm" variant="secondary" onClick={() => (change({ content: draft.text }), setDraft(null))}>
                            Restore draft
                        </Button>
                        <Button size="sm" variant="ghost" onClick={() => (clearDraft(draftKey), setDraft(null))}>
                            Discard
                        </Button>
                    </div>
                )}
                <div className="grid grid-cols-1 gap-16 md:grid-cols-2">
                    <TextField
                        label="Name"
                        mono
                        locked={!creating}
                        value={creating ? name : slug}
                        onChange={(e) => setName(e.target.value)}
                        helper={creating ? (finalName ? `Invoked as /${finalName}` : 'Letters, numbers and dashes') : 'Fixed once created'}
                    />
                    <TextField label="Description" value={form.description} onChange={(e) => change({ description: e.target.value })} helper="One line: when the agent should reach for it." />
                </div>
                <KeywordInput label="Keywords" value={form.keywords} onChange={(keywords) => change({ keywords })} placeholder="Add a keyword" />
                <MarkdownEditor look="full" outline value={form.content} onChange={(content) => change({ content })} onSaveShortcut={() => void save()} className="h-640" />
            </div>

            <Dialog
                open={conflict !== null}
                title="This skill changed while you were editing"
                message="Someone saved it after you opened it. Reload theirs to drop your edit, or keep yours to save it over theirs."
                confirmLabel="Keep mine"
                cancelLabel="Reload theirs"
                onConfirm={() => {
                    const hash = conflict?.base_hash
                    setConflict(null)
                    void save(hash)
                }}
                onCancel={() => {
                    if (conflict) {
                        const f = { description: conflict.description, keywords: conflict.keywords, content: conflict.content }
                        setLoaded(conflict)
                        setForm(f)
                        setBase(f)
                        clearDraft(draftKey)
                    }
                    setConflict(null)
                }}
            />
        </div>
    )
}
