import { Button, Card, ProfileHeader, Skeleton, TextField } from '@vizier/design'
import { useEffect, useState } from 'react'
import { myProfile, saveMyProfile, type Profile } from '../api/endpoints/auth'
import { PasswordCard } from '../screens/settings/PasswordCard'
import { SettingsFrame } from '../screens/shared/SettingsFrame'
import { useSession } from '../stores/session'
import { toast } from '../stores/toasts'

type Fields = { discord_id: string; discord_username: string; telegram_id: string; telegram_username: string; alias: string }

const toFields = (p: Profile): Fields => ({
    discord_id: p.discord_id ?? '',
    discord_username: p.discord_username ?? '',
    telegram_id: p.telegram_id ?? '',
    telegram_username: p.telegram_username ?? '',
    alias: p.alias.join(', '),
})

export default function SettingsProfile() {
    const user = useSession((s) => s.user)
    const [fields, setFields] = useState<Fields>()
    const [saved, setSaved] = useState<Fields>()
    const [busy, setBusy] = useState(false)
    const [problem, setProblem] = useState<string>()

    useEffect(() => {
        myProfile().then(
            (p) => {
                const f = toFields(p)
                setFields(f)
                setSaved(f)
            },
            (e) => setProblem(e instanceof Error ? e.message : 'Your profile could not be loaded.')
        )
    }, [])

    const set = (patch: Partial<Fields>) => setFields((f) => (f ? { ...f, ...patch } : f))
    const dirty = !!fields && !!saved && JSON.stringify(fields) !== JSON.stringify(saved)

    const save = async () => {
        if (!fields) return
        setBusy(true)
        try {
            const p = await saveMyProfile({
                discord_id: fields.discord_id.trim() || null,
                discord_username: fields.discord_username.trim() || null,
                telegram_id: fields.telegram_id.trim() || null,
                telegram_username: fields.telegram_username.trim() || null,
                alias: fields.alias.split(',').map((a) => a.trim()).filter(Boolean),
            })
            const f = toFields(p)
            setFields(f)
            setSaved(f)
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
            <Card title="Who you are on other channels" description="Agents use these to recognise you on Discord and Telegram, and to address you properly.">
                {problem ? (
                    <p className="type-small text-failed-text">{problem}</p>
                ) : !fields ? (
                    <Skeleton className="h-120" />
                ) : (
                    <div className="flex flex-col gap-16">
                        <div className="grid grid-cols-1 gap-16 sm:grid-cols-2">
                            <TextField label="Discord ID" mono value={fields.discord_id} onChange={(e) => set({ discord_id: e.target.value })} />
                            <TextField label="Discord username" value={fields.discord_username} onChange={(e) => set({ discord_username: e.target.value })} />
                            <TextField label="Telegram ID" mono value={fields.telegram_id} onChange={(e) => set({ telegram_id: e.target.value })} />
                            <TextField label="Telegram username" value={fields.telegram_username} onChange={(e) => set({ telegram_username: e.target.value })} />
                        </div>
                        <TextField label="Aliases" value={fields.alias} helper="Other names you go by, comma-separated." onChange={(e) => set({ alias: e.target.value })} />
                        <div>
                            <Button icon="save" loading={busy} disabled={!dirty} onClick={save}>
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
