import { Button, Dialog, MarkdownEditor, Notice, Skeleton, UnsavedIndicator } from '@vizier/design'
import { useCallback, useEffect, useState } from 'react'
import { useNavigate, useParams, useSearchParams } from 'react-router'
import { ApiError } from '../api/client'
import { coreRevision, getCore, saveCore } from '../api/endpoints/agentPages'
import { absoluteTime } from '../lib/format'
import { useMobile } from '../lib/useMobile'
import { PageFrame } from '../screens/shared/PageFrame'
import { useAgents } from '../stores/agents'
import { clearDraft, pendingDraft, saveDraft } from '../stores/drafts'
import { toast } from '../stores/toasts'

/** Editing CORE: the whole page is the editor. `?from=N` starts from revision N's text. */
export default function AgentCoreEdit() {
    const { agentId = '' } = useParams()
    const [params] = useSearchParams()
    const navigate = useNavigate()
    const mobile = useMobile()
    const agentName = useAgents((s) => s.agents.find((a) => a.agent_id === agentId)?.name)
    const key = `core:${agentId}`
    const from = Number(params.get('from')) || null
    const back = `/${agentId}/core`

    const [saved, setSaved] = useState<{ content: string; seq?: number } | null>(null)
    const [text, setText] = useState('')
    const [saving, setSaving] = useState(false)
    const [conflict, setConflict] = useState<{ content: string; seq: number } | null>(null)
    const [draft, setDraft] = useState<ReturnType<typeof pendingDraft>>(null)

    useEffect(() => {
        let live = true
        ;(async () => {
            const core = await getCore(agentId)
            if (!live) return
            setSaved({ content: core.content, seq: core.seq })
            const pending = pendingDraft(key, core.content)
            if (from !== null) {
                const rev = await coreRevision(agentId, from)
                if (live) setText(rev.content)
            } else if (pending) {
                // Coming back to an edit carries on with it.
                setText(pending.text)
                if (pending.stale) setDraft(pending)
            } else setText(core.content)
        })().catch((e) => toast.error("Couldn't load CORE", e.message))
        return () => {
            live = false
        }
    }, [agentId, key, from])

    const dirty = saved !== null && text !== saved.content
    const edit = (value: string) => {
        setText(value)
        if (saved) saveDraft(key, value, saved.content)
    }

    const write = useCallback(
        async (seq: number | undefined) => {
            setSaving(true)
            try {
                await saveCore(agentId, text, seq)
                clearDraft(key)
                toast.success('CORE saved')
                navigate(back)
            } catch (e) {
                if (e instanceof ApiError && e.status === 409) {
                    const data = e.data as { latest_seq: number; content: string }
                    setConflict({ content: data.content, seq: data.latest_seq })
                } else toast.error("Couldn't save CORE", e instanceof Error ? e.message : undefined)
            } finally {
                setSaving(false)
            }
        },
        [agentId, text, key, navigate, back]
    )

    const save = () => {
        if (!saved || saving || !dirty) return
        void write(saved.seq)
    }

    const discard = () => {
        clearDraft(key)
        navigate(back)
    }

    return (
        <PageFrame
            title={`Edit ${agentName ? `${agentName}’s ` : ''}CORE`}
            meta={from !== null ? `Starting from rev ${from} · saving makes it the new current version` : 'CORE.md'}
            back={{ href: back, label: 'Core' }}
            fill
            actions={
                <>
                    {dirty && <UnsavedIndicator>Unsaved · draft kept in this browser</UnsavedIndicator>}
                    <Button variant="ghost" onClick={discard}>
                        {dirty ? 'Discard' : 'Cancel'}
                    </Button>
                    <Button variant="primary" loading={saving} disabled={!dirty} shortcut="Ctrl S" onClick={save}>
                        Save
                    </Button>
                </>
            }
        >
            <div className="flex min-h-0 flex-1 flex-col gap-16 px-16 py-16 sm:px-24">
                {draft?.stale && (
                    <Notice
                        tone="info"
                        title={`Your edit from ${absoluteTime(draft.at)} was restored`}
                        actions={
                            <Button size="sm" variant="ghost" onClick={() => (clearDraft(key), setDraft(null), setText(saved?.content ?? ''))}>
                                Start from the current text
                            </Button>
                        }
                    >
                        CORE has changed since, so saving it would replace those changes.
                    </Notice>
                )}
                {saved === null ? (
                    <Skeleton lines={14} className="min-h-0 flex-1" />
                ) : (
                    // A phone has no room for source and preview side by side: it edits the source.
                    <MarkdownEditor
                        look="compact"
                        mode={mobile ? 'markdown' : undefined}
                        value={text}
                        onChange={edit}
                        onSaveShortcut={save}
                        className="min-h-[420px] flex-1"
                    />
                )}
            </div>

            <Dialog
                open={conflict !== null}
                title="CORE changed while you were editing"
                message={`Someone saved revision ${conflict?.seq ?? ''} after you opened it. Reload theirs to drop your edit, or keep yours to save it over theirs as a new revision.`}
                confirmLabel="Keep mine"
                cancelLabel="Reload theirs"
                onConfirm={() => {
                    const seq = conflict?.seq
                    setConflict(null)
                    void write(seq)
                }}
                onCancel={() => {
                    if (conflict) {
                        clearDraft(key)
                        setSaved({ content: conflict.content, seq: conflict.seq })
                        setText(conflict.content)
                    }
                    setConflict(null)
                }}
            />
        </PageFrame>
    )
}
