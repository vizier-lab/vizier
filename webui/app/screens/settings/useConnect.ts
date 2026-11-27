import type { ConnectAccount, ConnectConfirmProps } from '@vizier/design'
import { useCallback, useEffect, useRef, useState } from 'react'
import {
    previewConnection,
    redeemConnection,
    type ConnectPreview,
    type ConnectResult,
    type ConnectedAccount,
} from '../../api/endpoints/connections'
import { refusalOf } from '../../lib/connect'

type View = Pick<ConnectConfirmProps, 'state' | 'account' | 'username' | 'replaces' | 'message'>

const accountOf = (a: Omit<ConnectedAccount, 'linked_at'>): ConnectAccount => ({
    platform: a.platform,
    accountId: a.platform_user_id,
    handle: a.platform_username,
    displayName: a.platform_display_name,
})

const viewOf = (preview: ConnectPreview): View => ({
    state: 'ready',
    account: accountOf(preview),
    username: preview.vizier_username,
    replaces: preview.replaces ? accountOf(preview.replaces) : null,
})

/**
 * Reading a code and confirming it, as the `/connect` page and the Profile dialog both do. The
 * result is what `ConnectConfirm` draws; nothing is connected until `onConfirm`.
 */
export function useConnect(code: string | null, onConnected: (result: ConnectResult) => void) {
    const [view, setView] = useState<View>({ state: 'loading' })
    const [busy, setBusy] = useState(false)
    // The code last asked about: one request per code, and a late answer about another is dropped.
    const asked = useRef<string | null>(null)

    const load = useCallback(async (code: string) => {
        asked.current = code
        setView({ state: 'loading' })
        try {
            const preview = await previewConnection(code)
            if (asked.current === code) setView(viewOf(preview))
        } catch (error) {
            if (asked.current === code) setView(refusalOf(error))
        }
    }, [])

    useEffect(() => {
        if (!code) asked.current = null
        else if (asked.current !== code) void load(code)
    }, [code, load])

    const confirm = async () => {
        if (!code) return
        setBusy(true)
        try {
            onConnected(await redeemConnection(code))
        } catch (error) {
            const refusal = refusalOf(error)
            // A refusal that leaves the code usable stays on the confirmation, to try again.
            if (refusal.state === 'failed') setView((v) => ({ ...v, message: refusal.message }))
            else setView(refusal)
        } finally {
            setBusy(false)
        }
    }

    return {
        ...view,
        busy,
        onConfirm: () => void confirm(),
        onRetry: () => {
            if (code) void load(code)
        },
    }
}
