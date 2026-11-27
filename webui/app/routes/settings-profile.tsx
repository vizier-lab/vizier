import { Button, Card, ProfileHeader, Skeleton, TextField } from '@vizier/design'
import { useEffect, useState } from 'react'
import { myProfile, saveMyProfile } from '../api/endpoints/auth'
import { ConnectedAccountsCard } from '../screens/settings/ConnectedAccountsCard'
import { PasswordCard } from '../screens/settings/PasswordCard'
import { SettingsFrame } from '../screens/shared/SettingsFrame'
import { useSession } from '../stores/session'
import { toast } from '../stores/toasts'

export default function SettingsProfile() {
    const user = useSession((s) => s.user)
    const [aliases, setAliases] = useState<string>()
    const [saved, setSaved] = useState<string>()
    const [busy, setBusy] = useState(false)
    const [problem, setProblem] = useState<string>()

    const show = (names: string[]) => {
        setAliases(names.join(', '))
        setSaved(names.join(', '))
    }

    useEffect(() => {
        myProfile().then(
            (p) => show(p.alias),
            (e) => setProblem(e instanceof Error ? e.message : 'Your profile could not be loaded.')
        )
    }, [])

    const save = async () => {
        if (aliases === undefined) return
        setBusy(true)
        try {
            const p = await saveMyProfile({ alias: aliases.split(',').map((a) => a.trim()).filter(Boolean) })
            show(p.alias)
            toast.success('Profile saved')
        } catch (e) {
            toast.error('Couldn’t save your profile', e instanceof Error ? e.message : undefined)
        } finally {
            setBusy(false)
        }
    }

    return (
        <SettingsFrame title="Profile" description="How agents recognise you on Discord and Telegram, and the password you sign in with.">
            <ProfileHeader name={user?.username ?? ''} role={user?.role_name} />
            <ConnectedAccountsCard />
            <Card title="Other names">
                {problem ? (
                    <p className="type-small text-failed-text">{problem}</p>
                ) : aliases === undefined ? (
                    <Skeleton className="h-56" />
                ) : (
                    <div className="flex flex-col gap-16">
                        <TextField label="Aliases" value={aliases} helper="Other names you go by, comma-separated." onChange={(e) => setAliases(e.target.value)} />
                        <div>
                            <Button icon="save" loading={busy} disabled={aliases === saved} onClick={save}>
                                Save profile
                            </Button>
                        </div>
                    </div>
                )}
            </Card>
            <PasswordCard />
        </SettingsFrame>
    )
}
