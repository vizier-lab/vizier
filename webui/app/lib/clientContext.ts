// What this client says about itself, on every request it sends (`VizierRequest::client_context`).
//
// The agent cannot see the surface it is talking to: the same reply may be drawn here, read in a
// terminal, or pasted into Discord. This is how a channel says what it can render, so the agent
// spends a mermaid fence and a formula where they help and plain prose where they do not. The
// engine puts it at the head of the turn's context block — the first thing after the system
// prompts — and never stores it with the request, so it is a fact about now rather than a
// message the person typed.
import { declaredClientTools } from './clientTools'

/** At most `CLIENT_CONTEXT_MAX_ENTRIES` notes of `CLIENT_CONTEXT_MAX_BYTES` bytes each. */
export const CLIENT_CONTEXT: string[] = [
    'This client is the Vizier WebUI. It renders Markdown in full: a ```mermaid fence is drawn as a diagram, $…$ and $$…$$ as LaTeX, and other fenced code with syntax highlighting, wrapping long lines instead of scrolling sideways. A diagram or a formula is welcome where it says something prose cannot.',
]

/**
 * What every request frame carries: this client's tools and categories (client tools protocol
 * §1) and what it can render. Commands, reactions and tool results are not requests, so they
 * carry neither.
 */
export function withClientExtras(frame: object): object {
    if (!('content' in frame) || (frame as { content: object }).content === null) return frame
    const content = (frame as { content: object }).content
    if ('command' in content) return frame
    const declared = declaredClientTools()
    return {
        ...frame,
        client_context: CLIENT_CONTEXT,
        ...(declared.client_tools.length ? declared : {}),
    }
}
