import { Navigate } from 'react-router'

export default function RedirectUnknown() {
    return <Navigate to="/" replace />
}
