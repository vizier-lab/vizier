// The WebUI's client tool extension API (specs/019-client-tools/contracts/webui-extension-api.md).
//
// A client tool is declared on every request the WebUI sends, called by the agent from its
// scripts, answered here by `handle`, and drawn in the reply bubble by `render`, live and on
// reload. The WebUI's own tools (`render_interactive_ui`) register through this same API; an
// extension adds its own the same way.

import type { ReactNode } from 'react'
import type {
  ClientCallView,
  ClientCategoryDeclaration,
  ClientToolCallFrame,
  ClientToolDeclaration,
} from '../interfaces/types'

export type ClientToolDefinition = ClientToolDeclaration
export type { ClientCallView }

export interface ClientCallContext {
  callId: string
  agentId: string
  topicId: string
  /** Start an agent turn from this call (protocol §5). Throttled by the caller's own rules. */
  sendEvent(text: string, data?: unknown): void
  /** Save this call's state (protocol §6). */
  saveState(state: unknown): void
}

export interface ClientToolRegistration {
  definition: ClientToolDefinition
  /** Runs once, on the connection that declared the tool, when the agent calls it. */
  handle(args: Record<string, unknown>, ctx: ClientCallContext): Promise<unknown> | unknown
  /** Draws a call: live, on reload, in other tabs and browsers. Optional. */
  render?: (call: ClientCallView, ctx: ClientCallContext) => ReactNode
  /** Draw only once the call's turn has ended (answered, failed or aborted). */
  renderAfterTurn?: boolean
  /** Of this tool's calls in one turn, draw only the last. */
  latestPerTurn?: boolean
}

const DEFAULT_CATEGORY = 'client'

const tools = new Map<string, ClientToolRegistration>()
const categories = new Map<string, ClientCategoryDeclaration>()

const keyOf = (category: string | undefined, name: string) => `${category ?? DEFAULT_CATEGORY}/${name}`

/** Register a tool; a second one with the same `(category, name)` replaces the first. */
export function registerClientTool(registration: ClientToolRegistration): () => void {
  const key = keyOf(registration.definition.category, registration.definition.name)
  tools.set(key, registration)
  return () => {
    if (tools.get(key) === registration) tools.delete(key)
  }
}

export function registerClientCategory(category: ClientCategoryDeclaration): () => void {
  categories.set(category.name, category)
  return () => {
    if (categories.get(category.name) === category) categories.delete(category.name)
  }
}

/** What every request declares: the registered tools, and the categories they use. */
export function declaredClientTools(): {
  client_tools: ClientToolDeclaration[]
  client_categories: ClientCategoryDeclaration[]
} {
  const declared = [...tools.values()].map((registration) => registration.definition)
  const used = new Set(declared.map((tool) => tool.category ?? DEFAULT_CATEGORY))
  return {
    client_tools: declared,
    client_categories: [...categories.values()].filter((category) => used.has(category.name)),
  }
}

/** The registration for a call, by the client's own names. */
export function findClientTool(category: string, tool: string): ClientToolRegistration | undefined {
  return tools.get(keyOf(category, tool))
}

/** Run a live call's `handle`, as the `client_tool_result` payload to send back. */
export async function handleClientCall(
  frame: ClientToolCallFrame['client_tool_call'],
  ctx: ClientCallContext
): Promise<{ ok: unknown } | { error: string }> {
  const registration = findClientTool(frame.category, frame.tool)
  if (!registration) return { error: `this client does not provide ${frame.tool}` }
  try {
    const result = await registration.handle(frame.arguments ?? {}, ctx)
    // `undefined` is not JSON; a handle with nothing to say answers `null`.
    return { ok: result === undefined ? null : result }
  } catch (err) {
    return { error: err instanceof Error ? err.message : String(err) }
  }
}

/**
 * The calls of one turn to draw, by their tools' display options (research R19): a
 * `renderAfterTurn` tool's calls are left out while the turn is `running`, and of a
 * `latestPerTurn` tool's calls only the last is kept.
 */
export function visibleClientCalls(calls: ClientCallView[], running: boolean): ClientCallView[] {
  const shown = calls.filter((call) => !(running && findClientTool(call.category, call.tool)?.renderAfterTurn))
  return shown.filter(
    (call, at) =>
      !findClientTool(call.category, call.tool)?.latestPerTurn ||
      !shown.slice(at + 1).some((later) => later.category === call.category && later.tool === call.tool)
  )
}
