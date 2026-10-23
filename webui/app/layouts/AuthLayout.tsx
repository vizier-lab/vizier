import { Outlet } from 'react-router'

/** The Login/Onboarding frame. The LED-board panel arrives with those screens (US5). */
export default function AuthLayout() {
    return (
        <div className="grid min-h-screen grid-cols-1 bg-ground text-fg lg:grid-cols-2">
            <aside className="hidden bg-surface lg:block" aria-hidden />
            <main className="flex items-center justify-center p-32">
                <div className="w-full max-w-sm">
                    <Outlet />
                </div>
            </main>
        </div>
    )
}
