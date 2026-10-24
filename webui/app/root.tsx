import { Links, Meta, Outlet, Scripts, ScrollRestoration } from 'react-router'
import { MotionProvider } from '@vizier/design'
import './app.css'
import './lib/clientTools/builtin'

// Applies the stored theme before first paint; no stored choice follows the OS.
const themeScript = `try{var t=localStorage.getItem('vizier.theme');if(t==='dark'||t==='light')document.documentElement.setAttribute('data-theme',t)}catch(e){}`

export function Layout({ children }: { children: React.ReactNode }) {
    return (
        <html lang="en">
            <head>
                <meta charSet="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <title>Vizier</title>
                <script dangerouslySetInnerHTML={{ __html: themeScript }} />
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
    return <Outlet />
}
