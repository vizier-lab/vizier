import { NavItem, NavSection, SettingsNav, type SettingsPage } from '@vizier/design'
import { transport } from '../../api/transport'
import { useSession } from '../../stores/session'

/** The Settings rail, plus Sharing and Desktop groups when applicable. */
export function SettingsSidebar({ active, admin }: { active: string; admin: boolean }) {
    const desktop = transport.authMode === 'desktop'
    const can = useSession((s) => s.can)
    const canShare = can('owned_agents:share_conversations') || can('all_agents:share_conversations')
    const nav = <SettingsNav active={active as SettingsPage} hrefFor={(p) => `/settings/${p}`} showAdministration={admin} />
    return (
        <div className="flex h-full w-256 shrink-0 flex-col bg-shell">
            <div className="shrink-0 [&>nav]:h-auto">{nav}</div>
            {canShare && (
                <nav aria-label="Sharing settings" className="pr-12 pb-8">
                    <NavSection label="Sharing">
                        <NavItem icon="link" label="My shares" href="/settings/shares" active={active === 'shares'} />
                    </NavSection>
                </nav>
            )}
            {desktop && (
                <nav aria-label="Desktop settings" className="pr-12 pb-12">
                    <NavSection label="Desktop">
                        <NavItem icon="settings" label="Desktop" href="/settings/desktop" active={active === 'desktop'} />
                    </NavSection>
                </nav>
            )}
        </div>
    )
}
