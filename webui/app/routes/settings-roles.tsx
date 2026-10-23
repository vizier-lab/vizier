import { Button, Dialog, EmptyState, FilterTabs, Pagination, RoleRow, SearchField, Skeleton, SortSelect } from '@vizier/design'
import { useState } from 'react'
import { useNavigate } from 'react-router'
import { createRole, type Role } from '../api/endpoints/auth'
import { count } from '../lib/format'
import { usePagedList } from '../lib/usePagedList'
import { RoleNameForm } from '../screens/settings/RoleNameForm'
import { SettingsFrame } from '../screens/shared/SettingsFrame'

const PER_PAGE = 25

const SORTS = [
    { value: 'people', label: 'People' },
    { value: 'name', label: 'Name' },
]

/** Every role, across the page. A role opens on its own page, where its permissions are. */
export default function SettingsRoles() {
    const navigate = useNavigate()
    const list = usePagedList<Role>('/auth/roles', { sort: 'people' }, PER_PAGE)
    const [naming, setNaming] = useState(false)
    const counts = list.data?.counts
    const items = list.data?.items ?? []
    const open = (id: string) => navigate(`/settings/roles/${encodeURIComponent(id)}`)

    return (
        <SettingsFrame
            width="full"
            title="Roles"
            description="Permissions are granted to roles, never to individual people. Built-in roles can’t be edited."
            actions={
                <Button variant="primary" icon="plus" onClick={() => setNaming(true)}>
                    New role
                </Button>
            }
        >
            <div className="flex flex-wrap items-center gap-12">
                <div className="min-w-180 max-w-md flex-1">
                    <SearchField aria-label="Search roles" placeholder="Search roles" value={list.search} onValueChange={list.setSearch} count={counts ? count(counts.all ?? 0) : undefined} />
                </div>
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
                <span className="flex-1" />
                <SortSelect label="Sort" value={list.state.sort || 'people'} options={SORTS} onChange={list.setSort} />
            </div>
            {list.error ? (
                <EmptyState kind="error" title="Couldn’t load roles" message={list.error.message} />
            ) : !list.data ? (
                <Skeleton lines={4} />
            ) : items.length === 0 ? (
                <EmptyState kind="no-results" title="No role matched" message="Try a different search or filter." />
            ) : (
                <div className="flex flex-col gap-12">
                    <div className="flex flex-col divide-y divide-border rounded-lg border border-border" role="list" aria-label="Roles">
                        {items.map((r) => (
                            <RoleRow
                                key={r.role_id}
                                role="listitem"
                                name={r.name}
                                builtIn={r.is_system}
                                permissions={r.is_system ? 'all' : r.permissions.length}
                                people={r.people}
                                onSelect={() => open(r.role_id)}
                            />
                        ))}
                    </div>
                    <Pagination page={list.state.page} pageCount={list.pageCount} total={list.data.total} perPage={list.perPage} onPageChange={list.setPage} />
                </div>
            )}

            <Dialog open={naming} title="New role" confirmLabel="" onConfirm={() => {}} onCancel={() => setNaming(false)}>
                {naming && (
                    <RoleNameForm
                        initial=""
                        onCancel={() => setNaming(false)}
                        onCreate={async (name) => {
                            const made = await createRole(name, [])
                            setNaming(false)
                            open(made.role_id)
                        }}
                    />
                )}
            </Dialog>
        </SettingsFrame>
    )
}
