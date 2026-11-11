import { describe, expect, it } from 'vitest'
import { detailsAccess, detailsHref, detailsPath } from './agentDetails'

describe('detailsAccess', () => {
    it('shows every page to a person with every right', () => {
        expect(detailsAccess({ viewArtifacts: true, manageSecrets: true, editAgent: true })).toEqual({ hidden: [], disabled: [] })
    })

    it('hides Artifacts and Secrets, and disables Config, as the agent menu does', () => {
        expect(detailsAccess({ viewArtifacts: false, manageSecrets: false, editAgent: false })).toEqual({
            hidden: ['artifacts', 'secrets'],
            disabled: ['config'],
        })
    })
})

describe('detailsHref', () => {
    it('sends Core to the identity memory', () => {
        expect(detailsHref('archivist', 'core')).toBe('/archivist/memory/core/soul')
    })

    it('sends every other row to its page', () => {
        expect(detailsHref('archivist', 'tasks')).toBe('/archivist/tasks')
        expect(detailsHref('archivist', 'dreams')).toBe('/archivist/dream')
        expect(detailsHref('archivist', 'mcp')).toBe('/archivist/integrations/mcp')
    })

    it('names the details page', () => {
        expect(detailsPath('archivist')).toBe('/archivist/details')
    })
})
