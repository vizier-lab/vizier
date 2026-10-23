import { Button, Card, Dialog, EmptyState, Eyebrow, FilterTabs, Notice, PermissionGroup, PermissionRow, Pagination, RoleRow, SearchField, Skeleton, SortSelect, TextField } from '@vizier/design'
import { useEffect, useMemo, useState } from 'react'
import { Link, useSearchParams } from 'react-router'
import { paged } from '../api/client'
import { availablePermissions, createRole, deleteRole, updateRole, type Role } from '../api/endpoints/auth'
import { count } from '../lib/format'
import { groupsFor, permissionCount } from '../lib/permissions'
import { usePagedList } from '../lib/usePagedList'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { SettingsFrame } from '../screens/shared/SettingsFrame'
import { toast } from '../stores/toasts'

const PER_PAGE = 8

const SORTS = [
    { value: 'people', label: 'People' },
    { value: 'name', label: 'Name' },
]

export default function SettingsRoles() {
    const [params, setParams] = useSearchParams()
    const list = usePagedList<Role>('/auth/roles', { sort: 'people' }, PER_PAGE)
    const [available, setAvailable] = useState<string[]>([])
    const [naming, setNaming] = useState<{ from?: Role } | null>(null)
    const [deleting, setDeleting] = useState<Role | null>(null)

    useEffect(() => {
        availablePermissions().then(setAvailable, () => setAvailable([]))
    }, [])

    const selectedId = params.get('role')
    const select = (id: string | null) =>
        setParams(
            (p) => {
                const next = new URLSearchParams(p)
                if (id) next.set('role', id)
                else next.delete('role')
                return next
            },
            { replace: true }
        )

    const items = list.data?.items ?? []
    // The open role can be on another page of the list: it is fetched alone, never assumed.
    const [loose, setLoose] = useState<Role>()
    const selected = items.find((r) => r.role_id === selectedId) ?? (loose?.role_id === selectedId ? loose : undefined)
    useEffect(() => {
        if (!selectedId || items.some((r) => r.role_id === selectedId) || loose?.role_id === selectedId) return
        let live = true
        paged<Role>('/auth/roles', { limit: 100, sort: 'people' }).then((p) => live && setLoose(p?.items.find((r) => r.role_id === selectedId)))
        return () => {
            live = false
        }
    }, [selectedId, items, loose])

    const counts = list.data?.counts
    const total = list.data?.total ?? 0

    return (
        <SettingsFrame
            width="wide"
            title="Roles"
            description="Permissions are granted to roles, never to individual people. Built-in roles can’t be edited."
            actions={
                <Button variant="primary" icon="plus" onClick={() => setNaming({})}>
                    New role
                </Button>
            }
        >
            <div className="grid grid-cols-1 items-start gap-24 lg:grid-cols-[360px_minmax(0,1fr)]">
                <Card>
                    <div className="flex flex-col gap-12">
                        <SearchField aria-label="Search roles" placeholder={`Search ${count(counts?.all ?? 0)} roles`} value={list.search} onValueChange={list.setSearch} />
                        <FilterTabs
                            aria-label="Filter roles"
                            value={list.state.filter}
                            onChange={list.setFilter}
                            options={[
                                { value: 'all', label: 'All', count: counts ? count(counts.all ?? 0) : undefined },
                                { value: 'builtin', label: 'Built-in', count: counts ? count(counts.builtin ?? 0) : undefined },
                                { value: 'custom', label: 'Custom', count: counts ? count(counts.custom ?? 0) : undefined },
                            ]}
                        />
                        <SortSelect label="Sorted by" value={list.state.sort || 'people'} options={SORTS} onChange={list.setSort} />
                        {list.error ? (
                            <EmptyState kind="error" title="Couldn’t load roles" message={list.error.message} />
                        ) : !list.data ? (
                            <Skeleton className="h-120" />
                        ) : items.length === 0 ? (
                            <EmptyState kind="no-results" title="No role matched" message="Try a different search or filter." />
                        ) : (
                            <div className="flex flex-col" role="list" aria-label="Roles">
                                {items.map((r) => (
                                    <RoleRow
                                        key={r.role_id}
                                        role="listitem"
                                        name={r.name}
                                        builtIn={r.is_system}
                                        permissions={r.is_system ? 'all' : r.permissions.length}
                                        people={r.people}
                                        selected={r.role_id === selectedId}
                                        onSelect={() => select(r.role_id)}
                                    />
                                ))}
                            </div>
                        )}
                        <Pagination compact page={list.state.page} pageCount={list.pageCount} total={total} perPage={list.perPage} onPageChange={list.setPage} />
                    </div>
                </Card>

                {selected ? (
                    <RoleEditor
                        key={selected.role_id}
                        role={selected}
                        available={available}
                        onSaved={() => list.reload()}
                        onDuplicate={() => setNaming({ from: selected })}
                        onDelete={() => setDeleting(selected)}
                    />
                ) : (
                    <Card>
                        <EmptyState kind="empty" icon="shield" title="Pick a role" message="Choose one on the left to see what it grants, or make a new one." />
                    </Card>
                )}
            </div>

            <Dialog open={naming !== null} title={naming?.from ? `Duplicate ${naming.from.name}` : 'New role'} confirmLabel="" onConfirm={() => {}} onCancel={() => setNaming(null)}>
                {naming && (
                    <NameForm
                        initial={naming.from ? `${naming.from.name}-copy` : ''}
                        onCancel={() => setNaming(null)}
                        onCreate={async (name) => {
                            const made = await createRole(name, naming.from?.permissions ?? [])
                            setNaming(null)
                            list.reload()
                            select(made.role_id)
                        }}
                    />
                )}
            </Dialog>

            <ConfirmDialog
                open={deleting !== null}
                title={`Delete ${deleting?.name ?? 'role'}?`}
                message={deleting && deleting.people > 0 ? `${deleting.people} ${deleting.people === 1 ? 'person holds' : 'people hold'} this role. Move them to another role first; a role someone holds can’t be deleted.` : 'No one holds it.'}
                confirmLabel="Delete role"
                onConfirm={async () => {
                    const role = deleting
                    setDeleting(null)
                    if (!role) return
                    try {
                        await deleteRole(role.role_id)
                        select(null)
                        list.reload()
                    } catch (e) {
                        toast.error('Couldn’t delete the role', e instanceof Error ? e.message : undefined)
                    }
                }}
                onCancel={() => setDeleting(null)}
            />
        </SettingsFrame>
    )
}

type EditorProps = { role: Role; available: string[]; onSaved: () => void; onDuplicate: () => void; onDelete: () => void }

function RoleEditor({ role, available, onSaved, onDuplicate, onDelete }: EditorProps) {
    const [granted, setGranted] = useState(() => new Set(role.permissions))
    const [onlyGranted, setOnlyGranted] = useState(false)
    const [busy, setBusy] = useState(false)
    const groups = useMemo(() => groupsFor(available), [available])
    const builtIn = role.is_system
    const dirty = !builtIn && (granted.size !== role.permissions.length || role.permissions.some((p) => !granted.has(p)))
    const shown = builtIn ? available.length : granted.size

    const toggle = (permission: string, on: boolean) =>
        setGranted((g) => {
            const next = new Set(g)
            if (on) next.add(permission)
            else next.delete(permission)
            return next
        })

    return (
        <Card>
            <div className="flex flex-col gap-16">
                <div className="flex items-start justify-between gap-16">
                    <div className="flex min-w-0 flex-col gap-4">
                        <h2 className="flex items-center gap-8 type-code-medium text-fg">
                            {role.name}
                            <span className="type-small text-faint">{builtIn ? 'built-in' : 'custom'}</span>
                        </h2>
                        <p className="type-small text-muted">
                            Held by {role.people} {role.people === 1 ? 'person' : 'people'}.{' '}
                            <Link className="text-fg underline" to={`/settings/users?filter=${encodeURIComponent(`role:${role.name}`)}`}>
                                See them in Users →
                            </Link>
                        </p>
                    </div>
                    <div className="flex shrink-0 gap-8">
                        <Button onClick={onDuplicate}>Duplicate</Button>
                        {!builtIn && (
                            <Button variant="destructive" onClick={onDelete}>
                                Delete
                            </Button>
                        )}
                    </div>
                </div>

                {builtIn && <Notice tone="info">Built-in roles can’t be edited. Duplicate it to make a custom role with the same permissions.</Notice>}

                <Eyebrow
                    count={`${shown} of ${available.length}`}
                    action={
                        <button type="button" className="rounded-xs type-small text-muted hover:text-fg" aria-pressed={onlyGranted} onClick={() => setOnlyGranted((v) => !v)}>
                            Show granted only
                        </button>
                    }
                >
                    Permissions
                </Eyebrow>

                <div className="flex flex-col gap-16">
                    {groups.map((g) => {
                        const items = g.items.filter((i) => !onlyGranted || builtIn || granted.has(i.permission))
                        if (items.length === 0) return null
                        return (
                            <PermissionGroup key={g.title} title={g.title}>
                                {items.map((i) => (
                                    <PermissionRow
                                        key={i.permission}
                                        label={i.label}
                                        permission={i.permission}
                                        granted={builtIn || granted.has(i.permission)}
                                        disabled={builtIn}
                                        onGrantedChange={(on) => toggle(i.permission, on)}
                                    />
                                ))}
                            </PermissionGroup>
                        )
                    })}
                </div>

                {!builtIn && (
                    <div className="flex items-center gap-16 border-t border-border pt-16">
                        <Button
                            variant="primary"
                            loading={busy}
                            disabled={!dirty}
                            onClick={async () => {
                                setBusy(true)
                                try {
                                    await updateRole(role.role_id, role.name, [...granted])
                                    toast.success('Role saved')
                                    onSaved()
                                } catch (e) {
                                    toast.error('Couldn’t save the role', e instanceof Error ? e.message : undefined)
                                } finally {
                                    setBusy(false)
                                }
                            }}
                        >
                            Save role
                        </Button>
                        <p className="type-small text-faint">
                            Applies to all {role.people} {role.people === 1 ? 'person' : 'people'} on their next request. {permissionCount(granted.size, available.length)} granted.
                        </p>
                    </div>
                )}
            </div>
        </Card>
    )
}

function NameForm({ initial, onCreate, onCancel }: { initial: string; onCreate: (name: string) => Promise<void>; onCancel: () => void }) {
    const [name, setName] = useState(initial)
    const [busy, setBusy] = useState(false)
    const [error, setError] = useState<string>()
    return (
        <form
            className="flex flex-col gap-16"
            onSubmit={async (e) => {
                e.preventDefault()
                setBusy(true)
                setError(undefined)
                try {
                    await onCreate(name.trim())
                } catch (err) {
                    setError(err instanceof Error ? err.message : 'The role could not be made.')
                } finally {
                    setBusy(false)
                }
            }}
        >
            {error && <Notice tone="failed" title="The role wasn’t made">{error}</Notice>}
            <TextField label="Name" mono value={name} autoFocus helper="Choose its permissions next." onChange={(e) => setName(e.target.value)} />
            <div className="flex justify-end gap-8">
                <Button type="button" variant="ghost" onClick={onCancel}>Cancel</Button>
                <Button type="submit" variant="primary" loading={busy} disabled={!name.trim()}>Create role</Button>
            </div>
        </form>
    )
}
