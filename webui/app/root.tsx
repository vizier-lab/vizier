import { useEffect } from 'react'
import { Links, Meta, Outlet, Scripts, ScrollRestoration, useNavigate } from 'react-router'
import { MotionProvider } from '@vizier/design'
// Before the app's own stylesheet, so `.katex-display`'s overflow rule can follow it.
import 'katex/dist/katex.min.css'
import './app.css'
import './lib/clientTools/builtin'
import { transport } from './api/transport'
import { bootstrapView } from './lib/desktop'
import { PRE_PAINT_SCRIPT, THEME_COLORS } from './lib/themes/apply'
import { Bootstrap } from './screens/desktop/Bootstrap'
import { RuntimeBanner } from './screens/desktop/RuntimeBanner'
import { useAppearance } from './stores/appearance'
import { useDesktop } from './stores/desktop'
import { useNotifications } from './stores/notifications'
import { useToasts } from './stores/toasts'


export function Layout({ children }: { children: React.ReactNode }) {
    return (
        <html lang="en">
            <head>
                <meta charSet="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover" />
                <title>Vizier</title>
                <link rel="icon" type="image/svg+xml" href="/favicon.svg" />
                <link rel="apple-touch-icon" href="/icons/icon-192.png" />
                <link rel="manifest" href="/manifest.webmanifest" />
                <meta name="theme-color" content={THEME_COLORS.dark} />
                <meta name="color-scheme" content="light dark" />
                {/* iOS ignores parts of the manifest. */}
                <meta name="mobile-web-app-capable" content="yes" />
                <meta name="apple-mobile-web-app-capable" content="yes" />
                <meta name="apple-mobile-web-app-status-bar-style" content="black-translucent" />
                <meta name="apple-mobile-web-app-title" content="Vizier" />
                {/* The stored scheme before first paint; nothing stored follows the OS. */}
                <script dangerouslySetInnerHTML={{ __html: PRE_PAINT_SCRIPT }} />
                <Meta />
                <Links />
            </head>
            <body className="bg-ground text-fg">
                <MotionProvider>{children}</MotionProvider>
                <ScrollRestoration />
                <Scripts />
            </body>
        </html>
    )
}

export default function App() {
    const navigate = useNavigate()
    const { desktop, ready, state, update, init } = useDesktop()

    useEffect(() => init(), [init])
    useEffect(() => useAppearance.getState().init(), [])

    // Notifications, `vizier://` links and the tray ask the window to go somewhere.
    useEffect(() => {
        if (!desktop) return
        let off: (() => void) | undefined
        let gone = false
        void transport.desktop!.onNavigate((href) => (href === '/notifications' ? useNotifications.getState().openPanel() : navigate(href))).then((fn) => {
            if (gone) fn()
            else {
                off = fn
                void transport.desktop!.ready()
            }
        })
        return () => {
            gone = true
            off?.()
        }
    }, [desktop, navigate])

    // A new version is offered once, as a toast whose action installs it.
    useEffect(() => {
        if (!update?.available) return
        useToasts.getState().push(
            {
                tone: 'info',
                title: `Vizier ${update.version ?? ''} is available`.replace('  ', ' '),
                message: 'Select to install and restart.',
                onSelect: () => void transport.desktop?.updateInstall(),
            },
            12000
        )
    }, [update?.version, update?.available]) // eslint-disable-line react-hooks/exhaustive-deps

    if (!ready) return null
    if (desktop && bootstrapView(state) !== 'app') return <Bootstrap state={state} />
    return (
        <>
            {desktop && <RuntimeBanner />}
            <Outlet />
        </>
    )
}
