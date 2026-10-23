import { Select, SettingsNav, type SettingsPage } from '@vizier/design'
import { Outlet, useLocation, useNavigate } from 'react-router'
import { useMobile } from '../lib/useMobile'
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
                        options={admin ? [...ACCOUNT, ...ADMIN] : ACCOUNT}
                        value={active}
                        onChange={(page) => navigate(`/settings/${page}`)}
                    />
                </div>
                <div className="flex min-h-0 min-w-0 flex-1 flex-col">
                    <Outlet />
                </div>
            </div>
        )
    }
    return (
        <div className="flex min-h-0 flex-1">
            <SettingsNav active={active} hrefFor={(p) => `/settings/${p}`} showAdministration={admin} />
            <div className="flex min-w-0 flex-1 flex-col">
                <Outlet />
            </div>
        </div>
    )
}
