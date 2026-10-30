import { Select, motionDurations, type SettingsPage } from '@vizier/design'
import { type ReactNode } from 'react'
import { motion } from 'motion/react'
import { Outlet, useLocation, useNavigate } from 'react-router'
import { useMobile } from '../lib/useMobile'
import { transport } from '../api/transport'
import { useSession } from '../stores/session'

const ACCOUNT: { value: SettingsPage; label: string }[] = [
    { value: 'profile', label: 'Profile' },
    { value: 'password', label: 'Password' },
    { value: 'api-keys', label: 'API keys' },
]
const ADMIN: { value: SettingsPage; label: string }[] = [
    { value: 'providers', label: 'Providers' },
    { value: 'users', label: 'Users' },
    { value: 'roles', label: 'Roles' },
]

/**
 * The panel beside the settings menu. Only the panel changes when the person
 * moves between settings pages: the menu is the frame, and it stays.
 */
function SettingsPanel({ pathname, children }: { pathname: string; children: ReactNode }) {
    return (
        <motion.div
            key={pathname}
            className="flex min-h-0 min-w-0 flex-1 flex-col"
            initial={{ opacity: 0.7 }}
            animate={{
                opacity: 1,
                transition: { duration: motionDurations.fast },
            }}
        >
            {children}
        </motion.div>
    )
}

export default function SettingsLayout() {
    const { pathname } = useLocation()
    const navigate = useNavigate()
    const mobile = useMobile()
    const can = useSession((s) => s.can)
    const active = (pathname.split('/')[2] as SettingsPage | undefined) ?? 'profile'
    const admin = can('settings:providers') || can('users:manage') || can('roles:manage')
    // A phone has no room for a second rail beside the content: the pages become one list above it.
    if (mobile) {
        return (
            <div className="flex min-h-0 flex-1 flex-col">
                <div className="border-b border-border bg-sidebar px-16 py-12">
                    <Select
                        aria-label="Settings page"
                        options={[...ACCOUNT, ...(admin ? ADMIN : []), ...(transport.authMode === 'desktop' ? [{ value: 'desktop' as SettingsPage, label: 'Desktop' }] : [])]}
                        value={active}
                        onChange={(page) => navigate(`/settings/${page}`)}
                    />
                </div>
                <SettingsPanel pathname={pathname}>
                    <Outlet />
                </SettingsPanel>
            </div>
        )
    }
    return (
        // The menu beside the page is the shell's own column (AppShell); only the panel is here.
        <div className="flex min-h-0 flex-1">
            <SettingsPanel pathname={pathname}>
                <Outlet />
            </SettingsPanel>
        </div>
    )
}
