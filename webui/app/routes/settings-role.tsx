import { Button, Dialog, EmptyState, Eyebrow, Notice, PermissionGroup, PermissionRow, Skeleton, UnsavedIndicator } from '@vizier/design'
import { useEffect, useMemo, useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router'
import { ApiError } from '../api/client'
import { availablePermissions, createRole, deleteRole, getRole, updateRole, type Role } from '../api/endpoints/auth'
import { groupsFor, permissionCount } from '../lib/permissions'
import { RoleNameForm } from '../screens/settings/RoleNameForm'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { PageFrame } from '../screens/shared/PageFrame'
import { toast } from '../stores/toasts'

/** One role on its own page: what it grants, laid out across the page, and who holds it. */
export default function SettingsRole() {
    const { roleId = '' } = useParams()
    const navigate = useNavigate()
    const [role, setRole] = useState<Role | null>(null)
    const [missing, setMissing] = useState(false)
    const [available, setAvailable] = useState<string[]>([])
    const [granted, setGranted] = useState<Set<string>>(new Set())
    const [onlyGranted, setOnlyGranted] = useState(false)
    const [busy, setBusy] = useState(false)
    const [naming, setNaming] = useState(false)
    const [deleting, setDeleting] = useState(false)

    useEffect(() => {
        availablePermissions().then(setAvailable, () => setAvailable([]))
    }, [])
    useEffect(() => {
        setRole(null)
        getRole(roleId).then(
            (r) => {
                setRole(r)
                setGranted(new Set(r.permissions))
            },
            (e) => {
                if (e instanceof ApiError && e.status === 404) setMissing(true)
                else toast.error('Couldn’t load the role', e instanceof Error ? e.message : undefined)
            }
        )
    }, [roleId])

    const groups = useMemo(() => groupsFor(available), [available])
    const back = { href: '/settings/roles', label: 'Roles' }
    if (missing) {
        return (
            <PageFrame title="Role" back={back}>
                <EmptyState kind="error" title="This role isn’t there" message="It may have been deleted. Every role is listed on Roles." />
            </PageFrame>
        )
    }

    const builtIn = !!role?.is_system
    const dirty = !!role && !builtIn && (granted.size !== role.permissions.length || role.permissions.some((p) => !granted.has(p)))
    const shown = builtIn ? available.length : granted.size
    const toggle = (permission: string, on: boolean) =>
        setGranted((g) => {
            const next = new Set(g)
            if (on) next.add(permission)
            else next.delete(permission)
            return next
        })

    const save = async () => {
        if (!role) return
        setBusy(true)
        try {
            await updateRole(role.role_id, role.name, [...granted])
            toast.success('Role saved')
            setRole({ ...role, permissions: [...granted] })
        } catch (e) {
            toast.error('Couldn’t save the role', e instanceof Error ? e.message : undefined)
        } finally {
            setBusy(false)
        }
    }

    return (
        <PageFrame
            width="wide"
            back={back}
            title={role?.name ?? 'Role'}
            meta={role ? `${builtIn ? 'built-in' : 'custom'} · held by ${role.people} ${role.people === 1 ? 'person' : 'people'} · ${permissionCount(shown, available.length)} granted` : undefined}
            actions={
                role && (
                    <>
                        {dirty && <UnsavedIndicator>Unsaved</UnsavedIndicator>}
                        <Button onClick={() => setNaming(true)}>Duplicate</Button>
                        {!builtIn && (
                            <>
                                <Button variant="ghost" icon="trash" onClick={() => setDeleting(true)}>
                                    Delete
                                </Button>
                                <Button variant="primary" loading={busy} disabled={!dirty} onClick={() => void save()}>
                                    Save role
                                </Button>
                            </>
                        )}
                    </>
                )
            }
        >
            {!role ? (
                <Skeleton lines={8} />
            ) : (
                <>
                    {builtIn ? (
                        <Notice tone="info">Built-in roles can’t be edited. Duplicate it to make a custom role with the same permissions.</Notice>
                    ) : (
                        <p className="type-small text-muted">
                            Changes apply to everyone who holds it on their next request.{' '}
                            <Link className="text-fg underline" to={`/settings/users?filter=${encodeURIComponent(`role:${role.name}`)}`}>
                                See who holds it in Users
                            </Link>
                        </p>
                    )}
                    <Eyebrow
                        count={`${shown} of ${available.length}`}
                        action={
                            !builtIn && (
                                <button type="button" className="rounded-xs type-small text-muted hover:text-fg" aria-pressed={onlyGranted} onClick={() => setOnlyGranted((v) => !v)}>
                                    {onlyGranted ? 'Show all' : 'Show granted only'}
                                </button>
                            )
                        }
                    >
                        Permissions
                    </Eyebrow>
                    <div className="-mt-8 grid grid-cols-1 items-start gap-x-32 gap-y-20 xl:grid-cols-2">
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
                </>
            )}

            <Dialog open={naming} title={`Duplicate ${role?.name ?? 'role'}`} confirmLabel="" onConfirm={() => {}} onCancel={() => setNaming(false)}>
                {naming && role && (
                    <RoleNameForm
                        initial={`${role.name}-copy`}
                        onCancel={() => setNaming(false)}
                        onCreate={async (name) => {
                            const made = await createRole(name, role.is_system ? available : [...granted])
                            setNaming(false)
                            navigate(`/settings/roles/${encodeURIComponent(made.role_id)}`)
                        }}
                    />
                )}
            </Dialog>

            <ConfirmDialog
                open={deleting}
                title={`Delete ${role?.name ?? 'role'}?`}
                message={role && role.people > 0 ? `${role.people} ${role.people === 1 ? 'person holds' : 'people hold'} this role. Move them to another role first; a role someone holds can’t be deleted.` : 'No one holds it.'}
                confirmLabel="Delete role"
                onConfirm={async () => {
                    setDeleting(false)
                    if (!role) return
                    try {
                        await deleteRole(role.role_id)
                        navigate('/settings/roles')
                    } catch (e) {
                        toast.error('Couldn’t delete the role', e instanceof Error ? e.message : undefined)
                    }
                }}
                onCancel={() => setDeleting(false)}
            />
        </PageFrame>
    )
}
