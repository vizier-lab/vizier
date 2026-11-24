import { Navigate } from 'react-router'

/** Password lives on the Profile page now; old links land there. */
export default function RedirectPassword() {
    return <Navigate to="/settings/profile" replace />
}
