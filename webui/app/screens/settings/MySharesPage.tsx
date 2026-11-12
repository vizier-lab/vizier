import { Button, IconButton, Notice } from '@vizier/design'
import { useCallback, useEffect, useState } from 'react'
import { getShare, listShares, revokeShare, sharePath, type ShareListItem } from '../../api/shares'
import { exportOfflineSnapshot } from '../../lib/share/export'
import { parseSnapshot } from '../../lib/share/schema'
import { ConfirmDialog } from '../shared/ConfirmDialog'
import { SettingsFrame } from '../shared/SettingsFrame'
import { useSession } from '../../stores/session'
import { toast } from '../../stores/toasts'

export function formatAudience(item: ShareListItem): string {
    switch (item.audience) {
        case 'public':
            return 'Anyone'
        case 'signed_in':
            return 'Signed in'
        case 'specific':
            return `${item.people_count} ${item.people_count === 1 ? 'person' : 'people'}`
    }
}

export const HIDDEN_TITLE = 'Title not shown'

type RowProps = {
    item: ShareListItem
    all: boolean
    onCopy: (shareId: string) => void
    onDownload: (item: ShareListItem) => void
    onRevoke: (item: ShareListItem) => void
}

/**
 * One share. Without a title the viewer is not in the audience (FR-043) and could not open it, so
 * there is no Open. Downloading the file is the author's alone (FR-036), so only in "My shares".
 */
export function ShareRow({ item, all, onCopy, onDownload, onRevoke }: RowProps) {
    const isRevoked = Boolean(item.revoked_at)
    const openable = item.title !== undefined
    return (
        <tr className="hover:bg-hover">
            <td className="px-16 py-12">
                {openable ? (
                    <div className="font-medium text-fg">{item.title}</div>
                ) : (
                    <div className="font-medium italic text-muted">{HIDDEN_TITLE}</div>
                )}
                <div className="type-mono-xs text-muted">{item.agent_name}</div>
            </td>
            {all && <td className="px-16 py-12 text-fg">{item.creator_name ?? '–'}</td>}
            <td className="px-16 py-12 text-fg">{formatAudience(item)}</td>
            <td className="px-16 py-12 text-muted">{item.include_trail ? 'yes' : 'no'}</td>
            <td className="px-16 py-12 text-muted">
                {new Date(item.created_at).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' })}
            </td>
            <td className="px-16 py-12 text-right">
                {isRevoked ? (
                    <span className="type-small italic text-muted">revoked</span>
                ) : (
                    <div className="flex items-center justify-end gap-8">
                        {openable && (
                            <a
                                href={sharePath(item.share_id)}
                                target="_blank"
                                rel="noopener noreferrer"
                                className="type-small text-fg underline underline-offset-4 hover:text-muted"
                            >
                                Open
                            </a>
                        )}
                        {openable && (
                            <Button variant="ghost" size="sm" onClick={() => onCopy(item.share_id)}>
                                Copy link
                            </Button>
                        )}
                        {openable && !all && (
                            <Button variant="ghost" size="sm" onClick={() => onDownload(item)}>
                                Download file
                            </Button>
                        )}
                        <Button variant="ghost" size="sm" onClick={() => onRevoke(item)}>
                            Revoke
                        </Button>
                    </div>
                )}
            </td>
        </tr>
    )
}

export function MySharesPage() {
    const can = useSession((s) => s.can)
    const isAdmin = can('all_agents:share_conversations')
    const [all, setAll] = useState(false)
    const [shares, setShares] = useState<ShareListItem[]>([])
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState<string>()
    const [revoking, setRevoking] = useState<ShareListItem | null>(null)
    const [revokingBusy, setRevokingBusy] = useState(false)

    const load = useCallback(async (showAll: boolean) => {
        setLoading(true)
        setError(undefined)
        try {
            const data = await listShares({ all: showAll })
            setShares(data)
        } catch (e) {
            setError(e instanceof Error ? e.message : 'Failed to load shares')
        } finally {
            setLoading(false)
        }
    }, [])

    useEffect(() => {
        load(all)
    }, [all, load])

    // The author's own copy of the snapshot as a file (FR-036): read through the same route a visitor uses.
    const handleDownload = async (item: ShareListItem) => {
        try {
            const data = await getShare(item.share_id)
            const parsed = parseSnapshot(data.snapshot)
            if (!parsed.ok) throw new Error('The saved snapshot could not be read.')
            const { filename } = await exportOfflineSnapshot(parsed.snapshot)
            toast.success(`Exported ${filename}`)
        } catch (e) {
            toast.error('Could not download the file', e instanceof Error ? e.message : undefined)
        }
    }

    const handleCopy = (shareId: string) => {
        const url = `${window.location.origin}/share/${shareId}`
        navigator.clipboard.writeText(url)
        toast.success('Link copied to clipboard')
    }

    const handleRevokeConfirm = async () => {
        if (!revoking) return
        setRevokingBusy(true)
        try {
            await revokeShare(revoking.share_id)
            toast.success('Share revoked')
            setRevoking(null)
            await load(all)
        } catch (e) {
            toast.error('Failed to revoke share', e instanceof Error ? e.message : undefined)
        } finally {
            setRevokingBusy(false)
        }
    }

    return (
        <SettingsFrame
            width="wide"
            title="Shared conversations"
            description="Manage links created to share conversations. Revoking a link stops visitors from opening it immediately."
            actions={
                isAdmin ? (
                    <div className="flex items-center gap-8">
                        <Button
                            variant={!all ? 'primary' : 'ghost'}
                            size="sm"
                            onClick={() => setAll(false)}
                        >
                            My shares
                        </Button>
                        <Button
                            variant={all ? 'primary' : 'ghost'}
                            size="sm"
                            onClick={() => setAll(true)}
                        >
                            All shares
                        </Button>
                    </div>
                ) : undefined
            }
        >
            {error && <Notice tone="failed" title="Could not load shares">{error}</Notice>}

            {loading ? (
                <div className="p-24 text-center type-small text-muted">Loading shared conversations…</div>
            ) : shares.length === 0 ? (
                <div className="flex flex-col items-center justify-center p-48 text-center">
                    <p className="type-body-strong text-fg">No shared conversations</p>
                    <p className="mt-4 type-small text-muted">
                        {all
                            ? 'No conversations have been shared across this instance yet.'
                            : 'You haven’t shared any conversations yet. Use the Share button in a chat to make a link.'}
                    </p>
                </div>
            ) : (
                <div className="overflow-x-auto rounded-lg border border-border">
                    <table className="w-full text-left type-small">
                        <thead className="border-b border-border bg-sidebar text-muted">
                            <tr>
                                <th className="px-16 py-12 font-medium">Title</th>
                                {all && <th className="px-16 py-12 font-medium">Author</th>}
                                <th className="px-16 py-12 font-medium">Who</th>
                                <th className="px-16 py-12 font-medium">Trail</th>
                                <th className="px-16 py-12 font-medium">Created</th>
                                <th className="px-16 py-12 font-medium text-right">Actions</th>
                            </tr>
                        </thead>
                        <tbody className="divide-y divide-border">
                            {shares.map((item) => (
                                <ShareRow key={item.share_id} item={item} all={all} onCopy={handleCopy} onDownload={handleDownload} onRevoke={setRevoking} />
                            ))}
                        </tbody>
                    </table>
                </div>
            )}

            <ConfirmDialog
                open={revoking !== null}
                title={revoking?.title ? `Revoke “${revoking.title}”?` : 'Revoke this share?'}
                message="Anyone with the link will no longer be able to open it. This cannot be undone."
                confirmLabel={revokingBusy ? 'Revoking…' : 'Revoke link'}
                onConfirm={handleRevokeConfirm}
                onCancel={() => setRevoking(null)}
            />
        </SettingsFrame>
    )
}
