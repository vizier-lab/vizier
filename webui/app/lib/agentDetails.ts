import type { AgentDetailsPage } from '@vizier/design'
import { PAGE_PATH } from '../screens/shell/pages'

/** What a person may do with one agent, as the access helpers answer it. */
export type AgentAccess = {
    viewArtifacts: boolean
    manageSecrets: boolean
    editAgent: boolean
}

/**
 * Which of an agent's pages its details hide and which it shows but cannot open: the same
 * rules as the desktop's agent column and menu (Artifacts without view, Secrets without
 * access, Config without the right to edit).
 */
export function detailsAccess(access: AgentAccess): { hidden: AgentDetailsPage[]; disabled: AgentDetailsPage[] } {
    const hidden: AgentDetailsPage[] = []
    if (!access.viewArtifacts) hidden.push('artifacts')
    if (!access.manageSecrets) hidden.push('secrets')
    return { hidden, disabled: access.editAgent ? [] : ['config'] }
}

/** Where a row of an agent's details leads. */
export function detailsHref(agentId: string, page: AgentDetailsPage): string {
    return `/${agentId}${PAGE_PATH[page]}`
}

/** An agent's details page. */
export const detailsPath = (agentId: string) => `/${agentId}/details`
