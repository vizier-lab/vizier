import { SettingsNav, type SettingsPage } from '@vizier/design'
import { Outlet, useLocation } from 'react-router'
import { useSession } from '../stores/session'

export default function SettingsLayout() {
    const { pathname } = useLocation()
    const can = useSession((s) => s.can)
    const active = (pathname.split('/')[2] as SettingsPage | undefined) ?? 'profile'
    const admin = can('settings:providers') || can('users:manage') || can('roles:manage')
    return (
        <div className="flex min-h-0 flex-1">
            <SettingsNav active={active} hrefFor={(p) => `/settings/${p}`} showAdministration={admin} />
            <div className="flex min-w-0 flex-1 flex-col">
                <Outlet />
            </div>
        </div>
    )
}
