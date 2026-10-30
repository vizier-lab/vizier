import { Button, Card, Dialog, Notice, Skeleton, Switch, TextField } from '@vizier/design'
import { useEffect, useState } from 'react'
import { Link } from 'react-router'
import { transport, type DesktopSettings } from '../api/transport'
import { errorCode, errorText, validPort } from '../lib/desktop'
import { SettingsFrame } from '../screens/shared/SettingsFrame'
import { toast } from '../stores/toasts'

function Row({ label, hint, control }: { label: string; hint?: string; control: React.ReactNode }) {
    return (
        <div className="flex items-center gap-16">
            <div className="flex min-w-0 flex-1 flex-col gap-2">
                <span className="type-ui-medium text-fg">{label}</span>
                {hint && <span className="type-ui text-muted">{hint}</span>}
            </div>
            {control}
        </div>
    )
}

export default function SettingsDesktop() {
    const shell = transport.desktop
    const [settings, setSettings] = useState<DesktopSettings>()
    const [port, setPort] = useState('')
    const [problem, setProblem] = useState<string>()
    const [needsPassword, setNeedsPassword] = useState(false)
    const [confirmNetwork, setConfirmNetwork] = useState(false)
    const [url, setUrl] = useState<string>()
    const [update, setUpdate] = useState<string>()
    const [checking, setChecking] = useState(false)

    useEffect(() => {
        if (!shell) return
        shell.settingsGet().then(
            (s) => {
                setSettings(s)
                setPort(String(s.web_access.port))
            },
            (e) => setProblem(errorText(e, 'The settings could not be loaded.'))
        )
        let off: (() => void) | undefined
        void shell.onWebAccess((s) => setUrl(s.running ? s.url : undefined)).then((fn) => (off = fn))
        return () => off?.()
    }, [shell])

    if (!shell) return null

    const save = async (next: DesktopSettings) => {
        setProblem(undefined)
        setNeedsPassword(false)
        try {
            const saved = await shell.settingsSet(next)
            setSettings(saved ?? next)
            if (saved) setPort(String(saved.web_access.port))
        } catch (e) {
            if (errorCode(e) === 'password_required') setNeedsPassword(true)
            else setProblem(errorText(e, 'The settings could not be saved.'))
        }
    }

    const web = settings?.web_access

    return (
        <SettingsFrame title="Desktop" description="How Vizier behaves on this computer.">
            <div className="flex flex-col gap-20">
                {problem && <Notice tone="failed" title="Something went wrong">{problem}</Notice>}
                {!settings ? (
                    <Skeleton className="h-96" />
                ) : (
                    <>
                        <Card title="Workspace" description="Where this app keeps agents, memory and files." aria-label="Workspace">
                            <Row
                                label={settings.workspace ?? 'Default workspace'}
                                control={
                                    <Button variant="secondary" onClick={() => void shell.openWorkspace().catch((e) => setProblem(errorText(e, 'The workspace could not be opened.')))}>
                                        Open another workspace
                                    </Button>
                                }
                            />
                        </Card>

                        <Card title="Window" aria-label="Window">
                            <Row
                                label="Close to tray"
                                hint="Closing the window keeps Vizier running so agents and tasks continue."
                                control={<Switch aria-label="Close to tray" checked={settings.close_to_tray} onCheckedChange={(v) => void save({ ...settings, close_to_tray: v })} />}
                            />
                        </Card>

                        <Card title="Updates" aria-label="Updates">
                            <Row
                                label="Check for updates on start"
                                control={
                                    <Switch
                                        aria-label="Check for updates on start"
                                        checked={settings.updates.check_on_start}
                                        onCheckedChange={(v) => void save({ ...settings, updates: { check_on_start: v } })}
                                    />
                                }
                            />
                            <div className="flex items-center gap-12">
                                <Button
                                    variant="secondary"
                                    loading={checking}
                                    onClick={async () => {
                                        setChecking(true)
                                        try {
                                            const info = await shell.updateCheck()
                                            setUpdate(info.available ? `Version ${info.version} is available.` : 'Vizier is up to date.')
                                            if (info.available) toast.info(`Vizier ${info.version} is available`)
                                        } catch (e) {
                                            setUpdate(errorText(e, 'The update check failed.'))
                                        } finally {
                                            setChecking(false)
                                        }
                                    }}
                                >
                                    Check for updates
                                </Button>
                                {update && <span className="type-ui text-muted" role="status">{update}</span>}
                                {update?.includes('available') && (
                                    <Button onClick={() => void shell.updateInstall()}>Install and restart</Button>
                                )}
                            </div>
                        </Card>

                        <Card title="Web access" description="Open Vizier in a browser too. Off by default." aria-label="Web access">
                            {needsPassword && (
                                <Notice tone="failed" title="Set a password first">
                                    Web access needs a password. <Link className="underline" to="/settings/password">Set a password</Link>.
                                </Notice>
                            )}
                            <Row
                                label="Allow access from a browser"
                                control={
                                    <Switch
                                        aria-label="Allow access from a browser"
                                        checked={web!.enabled}
                                        onCheckedChange={(v) => void save({ ...settings, web_access: { ...web!, enabled: v } })}
                                    />
                                }
                            />
                            <div className="flex items-end gap-12">
                                <TextField label="Port" inputMode="numeric" value={port} invalid={!validPort(Number(port))} onChange={(e) => setPort(e.target.value)} />
                                <Button
                                    variant="secondary"
                                    disabled={!validPort(Number(port)) || Number(port) === web!.port}
                                    onClick={() => void save({ ...settings, web_access: { ...web!, port: Number(port) } })}
                                >
                                    Apply
                                </Button>
                            </div>
                            <Row
                                label="Allow other devices on the network"
                                hint="Without this, only this computer can open it."
                                control={
                                    <Switch
                                        aria-label="Allow other devices on the network"
                                        checked={web!.allow_network}
                                        onCheckedChange={(v) => (v ? setConfirmNetwork(true) : void save({ ...settings, web_access: { ...web!, allow_network: false } }))}
                                    />
                                }
                            />
                            {url && (
                                <p className="type-ui text-muted" role="status">
                                    Running at <code>{url}</code>
                                </p>
                            )}
                        </Card>

                        <Dialog
                            open={confirmNetwork}
                            tone="danger"
                            title="Allow other devices?"
                            message="Traffic is not encrypted, and anyone on this network can reach the sign-in page. Use this only on a network you trust."
                            confirmLabel="Allow"
                            onConfirm={() => {
                                setConfirmNetwork(false)
                                void save({ ...settings, web_access: { ...web!, allow_network: true } })
                            }}
                            onCancel={() => setConfirmNetwork(false)}
                        />
                    </>
                )}
            </div>
        </SettingsFrame>
    )
}
