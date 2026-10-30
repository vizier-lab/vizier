import { NavItem, NavSection, SettingsNav, type SettingsPage } from '@vizier/design'
import { transport } from '../../api/transport'

/** The Settings rail, plus a Desktop group right under it when this window is the desktop app. */
export function SettingsSidebar({ active, admin }: { active: string; admin: boolean }) {
    const desktop = transport.authMode === 'desktop'
    const nav = <SettingsNav active={active as SettingsPage} hrefFor={(p) => `/settings/${p}`} showAdministration={admin} />
    if (!desktop) return nav
    return (
        <div className="flex h-full w-256 shrink-0 flex-col bg-shell">
            <div className="shrink-0 [&>nav]:h-auto">{nav}</div>
            <nav aria-label="Desktop settings" className="pr-12 pb-12">
                <NavSection label="Desktop">
                    <NavItem icon="settings" label="Desktop" href="/settings/desktop" active={active === 'desktop'} />
                </NavSection>
            </nav>
        </div>
    )
}
