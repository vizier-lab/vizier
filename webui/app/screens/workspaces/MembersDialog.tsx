import { Avatar, Badge, Button, Dialog, Notice, SearchField, Sigil } from '@vizier/design'
import { useEffect, useState } from 'react'
import {
    addMember,
    getCandidates,
    removeMember,
    setRole,
    type Candidates,
    type WorkspaceMember,
} from '../../api/endpoints/workspaces'
import { toast } from '../../stores/toasts'

type Props = {
    open: boolean
    workspaceId: string
    members: WorkspaceMember[]
    /** An owner, or someone who manages every Workspace. */
    canManage: boolean
    onClose: () => void
    /** Membership changed: read the Workspace again. */
    onChanged: () => void
}

const SEARCH_DELAY_MS = 200

const message = (e: unknown) => (e instanceof Error ? e.message : 'Something went wrong')

/**
 * Who is in a Workspace. Anyone in it can read this. An owner adds from a search (so it stays
 * usable with hundreds of people and agents), removes, and changes roles. Bringing an agent in
 * shares it with everyone here, so that asks first (FR-005).
 */
export function MembersDialog({ open, workspaceId, members, canManage, onClose, onChanged }: Props) {
    const [query, setQuery] = useState('')
    const [found, setFound] = useState<Candidates | null>(null)
    const [sharing, setSharing] = useState<{ id: string; name: string } | null>(null)
    const [busy, setBusy] = useState(false)

    // Closing forgets the search, so reopening starts from the member list.
    useEffect(() => {
        if (!open) {
            setQuery('')
            setFound(null)
            setSharing(null)
        }
    }, [open])

    useEffect(() => {
        const q = query.trim()
        if (!open || !canManage || !q) {
            setFound(null)
            return
        }
        let live = true
        const timer = setTimeout(() => {
            getCandidates(workspaceId, q)
                .then((c) => live && setFound(c))
                .catch(() => live && setFound(null))
        }, SEARCH_DELAY_MS)
        return () => {
            live = false
            clearTimeout(timer)
        }
    }, [open, canManage, workspaceId, query])

    const run = async (work: () => Promise<unknown>) => {
        setBusy(true)
        try {
            await work()
            setSharing(null)
            onChanged()
            setQuery('')
        } catch (e) {
            toast.error(message(e))
        } finally {
            setBusy(false)
        }
    }

    const owners = members.filter((m) => m.kind === 'person' && m.role === 'owner').length
    const who = (kind: WorkspaceMember['kind'], handle: string, name: string) => (
        <>
            {kind === 'agent' ? <Sigil size="sm" agentId={handle} /> : <Avatar name={handle} size="sm" square />}
            <span className="flex min-w-0 flex-col">
                <span className="truncate type-body-strong text-fg">{name}</span>
                <span className="truncate type-mono-xs text-faint">@{handle}</span>
            </span>
        </>
    )

    const memberRow = (m: WorkspaceMember) => (
        <li key={`${m.kind}:${m.handle}`} className="flex items-center gap-10 py-6">
            {who(m.kind, m.handle, m.name)}
            <span className="ml-auto flex items-center gap-8">
                {m.role === 'owner' && <Badge tone="rank">Owner</Badge>}
                {!m.available && <Badge tone="failed">deleted</Badge>}
                {canManage && m.kind === 'person' && !(m.role === 'owner' && owners <= 1) && (
                    <Button
                        variant="ghost"
                        size="sm"
                        disabled={busy}
                        onClick={() => run(() => setRole(workspaceId, m.handle, m.role === 'owner' ? 'member' : 'owner'))}
                    >
                        {m.role === 'owner' ? 'Make member' : 'Make owner'}
                    </Button>
                )}
                {canManage && !(m.kind === 'person' && m.role === 'owner' && owners <= 1) && (
                    <Button
                        variant="ghost"
                        size="sm"
                        disabled={busy}
                        aria-label={`Remove ${m.name}`}
                        onClick={() => run(() => removeMember(workspaceId, m.kind, m.handle))}
                    >
                        Remove
                    </Button>
                )}
            </span>
        </li>
    )

    const resultsFor = (c: Candidates) => (
        <div className="flex flex-col gap-12">
            <section aria-label="Agents found">
                <h3 className="type-label uppercase text-faint">Agents</h3>
                {c.agents.items.length === 0 && <p className="type-ui text-faint">No agent matches.</p>}
                <ul className="m-0 list-none p-0">
                    {c.agents.items.map((a) => (
                        <li key={a.id} className="flex items-center gap-10 py-6">
                            {who('agent', a.id, a.name)}
                            <span className="ml-auto flex items-center gap-8">
                                {a.member && <Badge tone="neutral">Member</Badge>}
                                {a.reason === 'cannot_edit' && (
                                    <span className="type-ui text-faint">You cannot edit this agent</span>
                                )}
                                {a.addable && (
                                    <Button size="sm" disabled={busy} onClick={() => setSharing({ id: a.id, name: a.name })}>
                                        Add
                                    </Button>
                                )}
                            </span>
                        </li>
                    ))}
                </ul>
                {c.agents.more > 0 && <p className="type-ui text-faint">{c.agents.more} more match. Type more to narrow it.</p>}
            </section>
            <section aria-label="People found">
                <h3 className="type-label uppercase text-faint">People</h3>
                {c.people.items.length === 0 && <p className="type-ui text-faint">No one matches.</p>}
                <ul className="m-0 list-none p-0">
                    {c.people.items.map((p) => (
                        <li key={p.username} className="flex items-center gap-10 py-6">
                            {who('person', p.username, p.name)}
                            <span className="ml-auto">
                                {p.member ? (
                                    <Badge tone="neutral">Member</Badge>
                                ) : (
                                    <Button size="sm" disabled={busy} onClick={() => run(() => addMember(workspaceId, 'person', p.username))}>
                                        Add
                                    </Button>
                                )}
                            </span>
                        </li>
                    ))}
                </ul>
                {c.people.more > 0 && <p className="type-ui text-faint">{c.people.more} more match. Type more to narrow it.</p>}
            </section>
        </div>
    )

    const agents = members.filter((m) => m.kind === 'agent')
    const people = members.filter((m) => m.kind === 'person')

    return (
        <Dialog open={open} title="Members" confirmLabel="" onConfirm={onClose} onCancel={onClose}>
            <div className="flex h-360 flex-col gap-12">
                {canManage && (
                    <SearchField
                        value={query}
                        onValueChange={setQuery}
                        aria-label="Search people and agents to add"
                        placeholder="Add a person or an agent"
                        autoFocus
                    />
                )}
                {sharing && (
                    <Notice tone="retrying" title={`Add ${sharing.name}?`}>
                        <p className="type-ui">
                            Everyone in this Workspace will be able to use its tools and integrations, spend its provider budget, and
                            see what it recalls from memory.
                        </p>
                        <div className="mt-8 flex gap-8">
                            <Button size="sm" disabled={busy} onClick={() => run(() => addMember(workspaceId, 'agent', sharing.id))}>
                                Add {sharing.name}
                            </Button>
                            <Button size="sm" variant="ghost" onClick={() => setSharing(null)}>
                                Cancel
                            </Button>
                        </div>
                    </Notice>
                )}
                <div className="min-h-0 flex-1 overflow-y-auto">
                    {query.trim() && found ? (
                        resultsFor(found)
                    ) : query.trim() ? (
                        <p className="type-ui text-faint">Searching…</p>
                    ) : (
                        <>
                            <section aria-label="Agents">
                                <h3 className="type-label uppercase text-faint">Agents · {agents.length}</h3>
                                <ul className="m-0 list-none p-0">{agents.map(memberRow)}</ul>
                            </section>
                            <section aria-label="People">
                                <h3 className="type-label uppercase text-faint">People · {people.length}</h3>
                                <ul className="m-0 list-none p-0">{people.map(memberRow)}</ul>
                            </section>
                        </>
                    )}
                </div>
            </div>
        </Dialog>
    )
}
