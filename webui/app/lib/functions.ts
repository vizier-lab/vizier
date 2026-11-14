import { ApiError } from '../api/client'
import type { FunctionAuthor, FunctionProblem } from '../api/endpoints/functions'

// How an agent's functions read on the page (spec 061). Pure, so the rules are tested.

/** People read the agent's name where the agent itself reads "you". */
export function authorLabel(author: FunctionAuthor | null | undefined, agentName = 'the agent'): string {
    if (!author) return ''
    switch (author.kind) {
        case 'agent':
            return agentName
        case 'dream':
            return 'a dream'
        case 'person':
            return author.name
    }
}

/** "switched off by alice". */
export function offNote(disabledBy: FunctionAuthor | null | undefined, agentName?: string): string {
    return disabledBy ? `switched off by ${authorLabel(disabledBy, agentName)}` : 'switched off'
}

/** The functions a definition calls that the agent no longer has, or none. */
export function missingCalls(attention: { missing: string[] } | null | undefined): string[] {
    return attention?.missing ?? []
}

/** The edit was refused because the function changed since the person opened it. */
export function isOutOfDate(e: unknown): e is ApiError {
    return e instanceof ApiError && e.status === 409 && e.code === 'out_of_date'
}

/** The version the server holds now, from an `out_of_date` refusal. */
export function currentVersionOf(e: unknown): number | undefined {
    if (!isOutOfDate(e)) return undefined
    const v = (e.data as { current_version?: unknown } | undefined)?.current_version
    return typeof v === 'number' ? v : undefined
}

/** Every problem of a `422 invalid`, or the error's own message as one. */
export function problemsOf(e: unknown): FunctionProblem[] {
    if (e instanceof ApiError && e.status === 422) {
        const problems = (e.data as { problems?: unknown } | undefined)?.problems
        if (Array.isArray(problems)) return problems as FunctionProblem[]
    }
    return [{ code: 'error', message: e instanceof Error && e.message ? e.message : 'It wasn’t saved.' }]
}

/** One line per problem: "line 2, column 40: expected ')'", with the nearest names. */
export function problemText(problem: FunctionProblem): string {
    const where = typeof problem.line === 'number' ? `line ${problem.line}, column ${problem.column}: ` : ''
    const near = Array.isArray(problem.did_you_mean) && problem.did_you_mean.length > 0 ? ` Did you mean ${problem.did_you_mean.join(', ')}?` : ''
    return `${where}${problem.message}${near}`
}

/** Function and category names: lower-case letters, digits and `_`, starting with a letter. */
export const NAME_PATTERN = /^[a-z][a-z0-9_]{0,63}$/

/** The schema as the editor shows it. */
export const schemaText = (schema: unknown) => JSON.stringify(schema ?? { type: 'object' }, null, 2)

/** The editor's schema text, or why it is not JSON. */
export function parseSchema(text: string): { ok: true; value: unknown } | { ok: false; message: string } {
    try {
        return { ok: true, value: JSON.parse(text) }
    } catch (e) {
        return { ok: false, message: `The input schema isn’t valid JSON: ${e instanceof Error ? e.message : String(e)}` }
    }
}
