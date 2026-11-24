import { Outlet } from 'react-router'

/** The frame of a shared conversation: public, no sign-in gate and no app shell (FR-024). */
export default function ShareLayout() {
    return (
        <div className="min-h-screen bg-ground text-fg">
            <Outlet />
        </div>
    )
}
