import { Navigate } from 'react-router'

export default function RedirectSettings() {
    return <Navigate to="/settings/profile" replace />
}
