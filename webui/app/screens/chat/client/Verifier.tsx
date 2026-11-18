// The off-screen verifier (specs/063-html-generative-ui contracts/verify-protocol.md, T043).
// Mounted once by the app shell: each check the server asked this tab for mounts the version in a
// hidden 1024×768 frame with the page's current state, collects its problems for `observe_ms`
// (the host reports `unresponsive` when the heartbeat stops for a second), sends
// `ui_verify_report` on the conversation's socket and unmounts. A closing tab sends nothing; the
// server's deadline covers that.

import { useEffect, useRef } from 'react'
import { isPaused } from '../../../lib/uiPage/mounting'
import { NEVER_STARTED, PAUSED, ProblemList, reportFrame, type VerifyCheck } from '../../../lib/uiPage/verify'
import { topicKey, useConnection } from '../../../stores/connection'
import { useTurns } from '../../../stores/turns'
import { useUiVerify } from '../../../stores/uiVerify'
import { SandboxedPage } from './SandboxedPage'

export function Verifier() {
    const checks = useUiVerify((s) => s.checks)
    return (
        <>
            {checks.map((check) => (
                <Check key={check.requestId} check={check} />
            ))}
        </>
    )
}

function Check({ check }: { check: VerifyCheck }) {
    const problems = useRef(new ProblemList())
    const started = useRef(false)
    const paused = isPaused(check.pageId, check.version)
    const state = useTurns((s) => s.topics[topicKey(check.agentId, check.topicId)]?.pageStates?.[check.pageId]?.state)

    useEffect(() => {
        const begun = performance.now()
        const finish = () => {
            if (paused) problems.current.add(PAUSED)
            else if (!started.current) problems.current.add(NEVER_STARTED)
            useConnection.getState().send(check.agentId, check.topicId, reportFrame(check, problems.current.problems, performance.now() - begun))
            useUiVerify.getState().done(check.requestId)
        }
        const timer = setTimeout(finish, paused ? 0 : check.observeMs)
        return () => clearTimeout(timer)
    }, [check, paused])

    if (paused) return null
    return (
        <SandboxedPage
            mode="verify"
            agentId={check.agentId}
            topicId={check.topicId}
            channel={check.channel}
            pageId={check.pageId}
            version={check.version}
            title={`Checking page ${check.pageId} v${check.version}`}
            state={state}
            onReady={() => {
                started.current = true
            }}
            onProblem={(problem) => problems.current.add(problem)}
        />
    )
}
