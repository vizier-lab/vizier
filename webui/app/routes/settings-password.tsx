import { PasswordCard } from '../screens/settings/PasswordCard'
import { SettingsFrame } from '../screens/shared/SettingsFrame'

export default function SettingsPassword() {
    return (
        <SettingsFrame title="Password" description="Change the password you sign in with.">
            <PasswordCard />
        </SettingsFrame>
    )
}
