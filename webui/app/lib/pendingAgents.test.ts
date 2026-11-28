import { describe, expect, it } from 'vitest'
import type { SetupState } from '../api/endpoints/agents'
import { pendingAgents } from './pendingAgents'

const setup = (agent_id: string, phase: SetupState['phase'], name = ''): SetupState =>
    ({ agent_id, name, phase, message: '', downloads: [] }) as SetupState

describe('pendingAgents', () => {
    it('keeps a creation after a reload, from the setup the server still reports', () => {
        const got = pendingAgents([], { nova: setup('nova', 'downloading', 'Nova') }, () => false)
        expect(got).toEqual([{ agentId: 'nova', name: 'Nova' }])
    })

    it('ignores a stored agent restarting and a finished setup', () => {
        const setups = { atlas: setup('atlas', 'starting'), done: setup('done', 'ready') }
        expect(pendingAgents([], setups, (id) => id === 'atlas')).toEqual([])
    })

    it('lists an agent once, with the name this tab gave it', () => {
        const got = pendingAgents(
            [{ agentId: 'nova', name: 'Nova' }],
            { nova: setup('nova', 'starting', 'nova-from-server') },
            () => false
        )
        expect(got).toEqual([{ agentId: 'nova', name: 'Nova' }])
    })
})
