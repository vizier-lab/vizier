import { Navigate } from 'react-router'

/**
 * A legacy address, for links and bookmarks made while creation was a page: it opens the dialog over
 * Home and goes no further. Every control in the app opens the dialog in place instead (see
 * `withNewAgent`), so pressing New agent never moves you off the screen you are on.
 */
export default function AgentNew() {
    return <Navigate to="/?new=1" replace />
}
