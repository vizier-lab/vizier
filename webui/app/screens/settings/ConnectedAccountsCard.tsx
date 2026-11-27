import { Button, Card, ConnectConfirm, ConnectedAccountRow, Dialog, Skeleton, TextField, connectConfirmTitle } from '@vizier/design'
import { useCallback, useEffect, useState, type FormEvent } from 'react'
import {
    listConnections,
    removeUnverified,
    unlinkConnection,
    type ConnectedAccount,
    type Connections,
    type Platform,
} from '../../api/endpoints/connections'
import { accountName, bareHandle, connectedSince, isCodeShaped, normalizeCode, platformName } from '../../lib/connect'
import { toast } from '../../stores/toasts'
import { useConnect } from './useConnect'

const PLATFORMS: Platform[] = ['discord', 'telegram']

const reason = (error: unknown) => (error instanceof Error ? error.message : undefined)

/** Settings › Profile: the Discord and Telegram accounts proven to be this person's (spec 069). */
export function ConnectedAccountsCard() {
    const [connections, setConnections] = useState<Connections>()
    const [problem, setProblem] = useState<string>()
    const [typed, setTyped] = useState('')
    const [code, setCode] = useState<string | null>(null)
    // The account being unlinked stays set while its dialog closes, so the words don't vanish.
    const [target, setTarget] = useState<ConnectedAccount>()
    const [asking, setAsking] = useState(false)
    const [unlinking, setUnlinking] = useState(false)
    const [removing, setRemoving] = useState<Platform>()

    const load = useCallback(
        () =>
            listConnections().then(
                (found) => {
                    setConnections(found)
                    setProblem(undefined)
                },
                (error) => setProblem(reason(error) ?? 'Your connected accounts could not be loaded.')
            ),
        []
    )

    useEffect(() => {
        void load()
    }, [load])

    const flow = useConnect(code, (result) => {
        setCode(null)
        setTyped('')
        toast.success(`${platformName(result.link.platform)} account connected`)
        void load()
    })

    const submit = (e: FormEvent) => {
        e.preventDefault()
        if (isCodeShaped(typed)) setCode(normalizeCode(typed))
    }

    const unlink = async () => {
        if (!target) return
        setUnlinking(true)
        try {
            await unlinkConnection(target.platform)
            setAsking(false)
            toast.success(`${platformName(target.platform)} account unlinked`)
            await load()
        } catch (error) {
            toast.error('Couldn’t unlink the account', reason(error))
        } finally {
            setUnlinking(false)
        }
    }

    const remove = async (platform: Platform) => {
        setRemoving(platform)
        try {
            await removeUnverified(platform)
            await load()
        } catch (error) {
            toast.error('Couldn’t remove it', reason(error))
        } finally {
            setRemoving(undefined)
        }
    }

    const row = (platform: Platform, found: Connections) => {
        const link = found.links.find((l) => l.platform === platform)
        if (link)
            return (
                <ConnectedAccountRow
                    key={platform}
                    platform={platform}
                    state="connected"
                    handle={bareHandle(link.platform_username)}
                    accountId={link.platform_user_id}
                    since={connectedSince(link.linked_at)}
                    onUnlink={() => {
                        setTarget(link)
                        setAsking(true)
                    }}
                />
            )
        const old = found.unverified.find((u) => u.platform === platform)
        if (old)
            return (
                <ConnectedAccountRow
                    key={platform}
                    platform={platform}
                    state="unverified"
                    handle={bareHandle(old.platform_username)}
                    accountId={old.platform_user_id}
                    busy={removing === platform}
                    onRemove={() => void remove(platform)}
                />
            )
        return <ConnectedAccountRow key={platform} platform={platform} state="none" />
    }

    return (
        <Card title="Connected accounts" description="Connect Discord and Telegram so agents know your messages are yours.">
            {problem ? (
                <div className="flex items-center gap-12">
                    <p className="type-small text-failed-text">{problem}</p>
                    <Button size="sm" onClick={() => void load()}>
                        Try again
                    </Button>
                </div>
            ) : !connections ? (
                <Skeleton className="h-120" />
            ) : (
                <div className="flex flex-col gap-8">{PLATFORMS.map((platform) => row(platform, connections))}</div>
            )}
            <form className="flex flex-col gap-6 border-t border-border pt-14" onSubmit={submit}>
                <div className="flex flex-wrap items-end gap-8">
                    <TextField
                        className="w-220"
                        label="Have a code?"
                        mono
                        placeholder="ABCD-EFGH"
                        autoComplete="off"
                        spellCheck={false}
                        value={typed}
                        onChange={(e) => setTyped(e.target.value)}
                    />
                    {/* Centred on the field's height, so the two sit on one line. */}
                    <div className="flex h-40 items-center">
                        <Button type="submit" variant="primary" disabled={!isCodeShaped(typed)}>
                            Connect
                        </Button>
                    </div>
                </div>
                <p className="type-small text-faint">Send /connect to a bot to get a code. A code works once, for 10 minutes.</p>
            </form>

            <Dialog
                open={code !== null}
                title={connectConfirmTitle(flow)}
                confirmLabel=""
                onConfirm={() => {}}
                onCancel={() => {
                    if (!flow.busy) setCode(null)
                }}
            >
                <ConnectConfirm {...flow} onCancel={() => setCode(null)} doneLabel="Close" />
            </Dialog>
            <Dialog
                open={asking}
                tone="danger"
                title={target ? `Unlink ${platformName(target.platform)}?` : 'Unlink?'}
                message={
                    target &&
                    `Messages from ${accountName(target)} will read as Unregistered User to every agent until you connect this account again.`
                }
                confirmLabel="Unlink"
                confirmLoading={unlinking}
                onConfirm={() => void unlink()}
                onCancel={() => {
                    if (!unlinking) setAsking(false)
                }}
            />
        </Card>
    )
}
