import { Badge, Dialog, Sigil, Avatar } from '@vizier/design'
import type { WorkspaceMember } from '../../api/endpoints/workspaces'

/**
 * Who is in a Workspace. Anyone in it can see this; adding and removing people and agents, which
 * starts from a search, is an owner's job and arrives with the management screen.
 */
export function MembersDialog({ open, members, onClose }: { open: boolean; members: WorkspaceMember[]; onClose: () => void }) {
    const agents = members.filter((m) => m.kind === 'agent')
    const people = members.filter((m) => m.kind === 'person')
    const row = (m: WorkspaceMember) => (
        <li key={`${m.kind}:${m.handle}`} className="flex items-center gap-10 py-6">
            {m.kind === 'agent' ? <Sigil size="sm" agentId={m.handle} /> : <Avatar name={m.handle} size="sm" square />}
            <span className="flex min-w-0 flex-col">
                <span className="truncate type-body-strong text-fg">{m.name}</span>
                <span className="truncate type-mono-xs text-faint">@{m.handle}</span>
            </span>
            {m.role === 'owner' && (
                <Badge tone="rank" className="ml-auto">
                    Owner
                </Badge>
            )}
            {!m.available && (
                <Badge tone="failed" className="ml-auto">
                    deleted
                </Badge>
            )}
        </li>
    )
    return (
        <Dialog open={open} title="Members" confirmLabel="" onConfirm={onClose} onCancel={onClose}>
            <div className="flex max-h-360 flex-col gap-12 overflow-y-auto">
                <section aria-label="Agents">
                    <h3 className="type-label uppercase text-faint">Agents · {agents.length}</h3>
                    <ul className="m-0 list-none p-0">{agents.map(row)}</ul>
                </section>
                <section aria-label="People">
                    <h3 className="type-label uppercase text-faint">People · {people.length}</h3>
                    <ul className="m-0 list-none p-0">{people.map(row)}</ul>
                </section>
            </div>
        </Dialog>
    )
}
