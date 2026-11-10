import { Notice } from '@vizier/design'

/**
 * The browser will not install the app on this address: a plain-HTTP address that is not
 * loopback is not a secure context, so the browser refuses the worker. The app says so
 * instead of trying and failing.
 */
export function InsecureNetworkNotice() {
    return (
        <Notice tone="info" title="Install is loopback-only">
            On this address the browser treats the connection as insecure and refuses the install. Use{' '}
            <code>http://127.0.0.1:&lt;port&gt;</code> on the computer Vizier runs on.
        </Notice>
    )
}
