import { Button, Dialog, IconButton, Notice, Select, TextField } from '@vizier/design'
import { useEffect, useMemo, useState } from 'react'
import { paged } from '../api/client'
import { createUser, deleteUser, updateUser, type Role, type UserSummary } from '../api/endpoints/auth'
import { absoluteTime } from '../lib/format'
import { usePagedList } from '../lib/usePagedList'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { PagedTable } from '../screens/shared/PagedTable'
import { SettingsFrame } from '../screens/shared/SettingsFrame'
import { useSession } from '../stores/session'
import { toast } from '../stores/toasts'

const SORTS = [
    { value: 'created', label: 'Newest' },
    { value: 'username', label: 'Username' },
]

/** The roles to pick from: the ones most people hold, which is all of them below a hundred. */
function useRoles(): Role[] {
    const [roles, setRoles] = useState<Role[]>([])
    useEffect(() => {
        paged<Role>('/auth/roles', { limit: 100, sort: 'people' }).then((p) => setRoles(p?.items ?? []), () => setRoles([]))
    }, [])
    return roles
}

export default function SettingsUsers() {
    const me = useSession((s) => s.user)
    const list = usePagedList<UserSummary>('/auth/users', { sort: 'created' })
    const roles = useRoles()
    const [adding, setAdding] = useState(false)
    const [deleting, setDeleting] = useState<UserSummary | null>(null)

    const roleOptions = (current?: UserSummary) => {
        const options = roles.map((r) => ({ value: r.role_id, label: r.name }))
        // A role beyond the first hundred stays visible on the person who has it.
        return current && !options.some((o) => o.value === current.role_id) ? [...options, { value: current.role_id, label: current.role_name ?? current.role_id }] : options
    }

    const filters = useMemo(
        () => [
            { value: 'all', label: 'All' },
            ...Object.keys(list.data?.counts ?? {})
                .filter((k) => k.startsWith('role:'))
                .map((k) => ({ value: k, label: k.slice(5) })),
        ],
        [list.data?.counts]
    )

    const changeRole = async (user: UserSummary, role_id: string) => {
        try {
            await updateUser(user.user_id, { role_id })
            list.reload()
        } catch (e) {
            toast.error('Couldn’t change the role', e instanceof Error ? e.message : undefined)
        }
    }

    return (
        <SettingsFrame
            title="Users"
            description="Everyone who can sign in to this Vizier. Their role decides what they can do."
            actions={
                <Button variant="primary" icon="plus" onClick={() => setAdding(true)}>
                    Add user
                </Button>
            }
        >
            <PagedTable<UserSummary>
                label="Users"
                searchLabel="Search users"
                list={list}
                rowKey={(u) => u.user_id}
                sorts={SORTS}
                filters={filters}
                columns={[{ label: 'Username' }, { label: 'Role', width: 200 }, { label: 'Created', width: 140 }, { label: '', width: 56, align: 'end' }]}
                endAligned={[3]}
                row={(u) => [
                    <span key="u" className="type-body-strong text-fg">
                        {u.username}
                        {u.user_id === me?.user_id && <span className="type-small text-faint"> (you)</span>}
                    </span>,
                    <Select key="r" look="badge" aria-label={`Role of ${u.username}`} options={roleOptions(u)} value={u.role_id} onChange={(v) => void changeRole(u, v)} />,
                    <span key="c" className="type-small text-muted" title={absoluteTime(u.created_at)}>{absoluteTime(u.created_at, true)}</span>,
                    u.user_id === me?.user_id ? null : <IconButton key="d" icon="trash" aria-label={`Delete ${u.username}`} onClick={() => setDeleting(u)} />,
                ]}
                empty={{ icon: 'users', title: 'No one else yet', message: 'Add someone to give them their own sign-in.' }}
            />

            <Dialog open={adding} title="Add user" confirmLabel="" onConfirm={() => {}} onCancel={() => setAdding(false)}>
                {adding && (
                    <AddUser
                        roles={roleOptions()}
                        onCancel={() => setAdding(false)}
                        onAdded={() => {
                            setAdding(false)
                            list.reload()
                            toast.success('User added')
                        }}
                    />
                )}
            </Dialog>

            <ConfirmDialog
                open={deleting !== null}
                title={`Delete ${deleting?.username ?? 'user'}?`}
                message="They can no longer sign in. Agents they own stay."
                confirmLabel="Delete user"
                onConfirm={async () => {
                    const user = deleting
                    setDeleting(null)
                    if (!user) return
                    try {
                        await deleteUser(user.user_id)
                        list.reload()
                    } catch (e) {
                        toast.error('Couldn’t delete the user', e instanceof Error ? e.message : undefined)
                    }
                }}
                onCancel={() => setDeleting(null)}
            />
        </SettingsFrame>
    )
}

function AddUser({ roles, onCancel, onAdded }: { roles: { value: string; label: string }[]; onCancel: () => void; onAdded: () => void }) {
    const [username, setUsername] = useState('')
    const [password, setPassword] = useState('')
    const [role, setRole] = useState('')
    const [busy, setBusy] = useState(false)
    const [error, setError] = useState<string>()
    const short = password !== '' && password.length < 8
    return (
        <form
            className="flex flex-col gap-16"
            onSubmit={async (e) => {
                e.preventDefault()
                setBusy(true)
                setError(undefined)
                try {
                    await createUser(username.trim(), password, role || undefined)
                    onAdded()
                } catch (err) {
                    setError(err instanceof Error ? err.message : 'The user could not be added.')
                } finally {
                    setBusy(false)
                }
            }}
        >
            {error && <Notice tone="failed" title="The user wasn’t added">{error}</Notice>}
            <TextField label="Username" value={username} autoComplete="off" autoFocus onChange={(e) => setUsername(e.target.value)} />
            <TextField label="Password" type="password" autoComplete="new-password" value={password} invalid={short} helper="At least 8 characters." onChange={(e) => setPassword(e.target.value)} />
            <Select label="Role" options={roles} value={role || undefined} placeholder="Choose a role" onChange={setRole} />
            <div className="flex justify-end gap-8">
                <Button type="button" variant="ghost" onClick={onCancel}>Cancel</Button>
                <Button type="submit" variant="primary" loading={busy} disabled={!username.trim() || password.length < 8}>Add user</Button>
            </div>
        </form>
    )
}
