// Checking a page version off-screen for the agent (specs/063-html-generative-ui
// contracts/verify-protocol.md, tasks T043): what a check is, the problems it collects and the
// report it sends. The mounting is `screens/chat/client/Verifier.tsx`.

import type { Problem } from './host'

/** A `ui_verify` request this tab was asked to answer. */
export type VerifyCheck = {
    requestId: string
    agentId: string
    topicId: string
    channel?: string
    pageId: string
    version: number
    observeMs: number
}

/** The server keeps 20; one more tells it there were more. */
export const MAX_REPORTED = 21

/** The problems seen during one check, in order, without repeats, capped. */
export class ProblemList {
    private readonly seen = new Set<string>()
    readonly problems: Problem[] = []

    add(problem: Problem) {
        const key = `${problem.kind}|${problem.message}|${problem.line ?? ''}|${problem.column ?? ''}`
        if (this.seen.has(key) || this.problems.length >= MAX_REPORTED) return
        this.seen.add(key)
        this.problems.push(problem)
    }
}

/** A frame that never said it runs: its document or the kit failed before the page could. */
export const NEVER_STARTED: Problem = {
    kind: 'kit',
    message: 'The page did not start: its frame never reported that it runs.',
    source: 'kit',
}

/** A version that froze this tab before: it is not mounted again to be checked. */
export const PAUSED: Problem = {
    kind: 'unresponsive',
    message: 'This version froze the tab last time it ran and is paused. Move heavy work into vizier.worker.',
    source: 'page',
}

/** The `ui_verify_report` frame. */
export function reportFrame(check: VerifyCheck, problems: Problem[], observedMs: number) {
    return { ui_verify_report: { request_id: check.requestId, problems, observed_ms: Math.round(observedMs) } }
}
