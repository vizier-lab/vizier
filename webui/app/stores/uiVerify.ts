// The page checks this tab was asked to run (spec 063 T043): queued by the topic socket's
// `ui_verify` frame, run by `screens/chat/client/Verifier.tsx`, removed once reported.

import { create } from 'zustand'
import type { VerifyCheck } from '../lib/uiPage/verify'

type UiVerifyStore = {
    checks: VerifyCheck[]
    add: (check: VerifyCheck) => void
    done: (requestId: string) => void
}

export const useUiVerify = create<UiVerifyStore>()((set) => ({
    checks: [],
    add: (check) =>
        set((cur) => (cur.checks.some((c) => c.requestId === check.requestId) ? cur : { checks: [...cur.checks, check] })),
    done: (requestId) => set((cur) => ({ checks: cur.checks.filter((c) => c.requestId !== requestId) })),
}))
