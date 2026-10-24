// Unit tests for the page-level access helpers. Run with:
//
//   cd webui && npx vitest run app/lib/access.test.ts
//
// Hiding a nav item is UX only: these helpers must agree with the server rule in
// `src/secrets/access.rs`, because a direct URL is refused there, not here.

import { beforeEach, test } from 'vitest'
import assert from 'node:assert/strict'

import { canManageSecrets } from './access.ts'
import { useSession } from '../stores/session.ts'

const person = (permissions: string[]) => ({
    user_id: 'user-1',
    username: 'alice',
    role_name: 'member',
    permissions,
})

const agent = (owner_id = 'user-1') => ({ owner_id })

beforeEach(() => {
    useSession.setState({ user: null })
})

test('no signed-in user can manage nothing', () => {
    assert.equal(canManageSecrets(agent()), false)
})

test('all_agents:secret manages any agent', () => {
    useSession.setState({ user: person(['all_agents:secret']) })
    assert.equal(canManageSecrets(agent('someone-else')), true)
})

test('owned_agents:secret manages an agent the person owns', () => {
    useSession.setState({ user: person(['owned_agents:secret']) })
    assert.equal(canManageSecrets(agent('user-1')), true)
})

test('owned_agents:secret does not manage another person’s agent', () => {
    useSession.setState({ user: person(['owned_agents:secret']) })
    assert.equal(canManageSecrets(agent('user-2')), false)
})

test('neither permission manages nothing, owned or not', () => {
    useSession.setState({ user: person(['all_agents:view', 'owned_agents:edit']) })
    assert.equal(canManageSecrets(agent('user-1')), false)
    assert.equal(canManageSecrets(agent('user-2')), false)
})

test('a revoked permission is gone from the session list', () => {
    useSession.setState({ user: person(['all_agents:secret']) })
    assert.equal(canManageSecrets(agent('user-2')), true)
    useSession.setState({ user: person([]) })
    assert.equal(canManageSecrets(agent('user-2')), false)
})
