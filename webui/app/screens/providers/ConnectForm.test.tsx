import { renderToString } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import type { ProviderSummary } from '../../api/endpoints/catalogue'
import type { SignInRequest, SignInResponse, SignInStarted, ssoSignIn } from '../../api/endpoints/providers'
import { absoluteTime } from '../../lib/format'
import type { SsoStateView } from '../../lib/providers'
import { ConnectForm, reSignInSignIn, SessionState, SignInPanel, signInStep, type SignInPanelState } from './ConnectForm'

type Provider = Parameters<typeof ConnectForm>[0]['provider']

/** The exact policy the server's guide carries (contract `provider-credentials-api.md` §1). */
const POLICY = `{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Action": ["bedrock:InvokeModel", "bedrock:ListFoundationModels"],
    "Resource": "*"
  }]
}`

const GUIDE: NonNullable<ProviderSummary['guide']> = {
    intro: 'Use an IAM access key pair with permission to invoke Bedrock models.',
    steps: [
        { title: 'Create an IAM access key', body: 'IAM → Users → Security credentials → Create access key.' },
        { title: 'Attach the minimal policy', body: 'Allow the actions below.' },
        { title: 'Enable model access', body: 'Bedrock → Model access.' },
        { title: 'Choose a region', body: 'Use a Bedrock region where the model is offered.' },
        { title: 'Paste the values', body: 'Enter the access key ID, secret access key and region.' },
    ],
    policy: { language: 'json', code: POLICY },
    links: [
        { label: 'AWS IAM users', url: 'https://console.aws.amazon.com/iam/home#/users' },
        { label: 'Bedrock model access', url: 'https://console.aws.amazon.com/bedrock/home#/modelaccess' },
        { label: 'AWS supported models', url: 'https://docs.aws.amazon.com/bedrock/latest/userguide/models-supported.html' },
        { label: 'Vizier provider docs', url: 'https://www.vizier.rs/docs/configuration/providers#amazon-bedrock' },
    ],
}

function bedrock(guide: ProviderSummary['guide']): Provider {
    return {
        id: 'amazon-bedrock',
        name: 'Amazon Bedrock',
        fields: ['access_key_id', 'secret_access_key', 'session_token', 'region'],
        endpoint: null,
        doc: null,
        guide,
    }
}

function draw(editing: boolean, guide: ProviderSummary['guide'] = GUIDE) {
    return renderToString(<ConnectForm provider={bedrock(guide)} editing={editing} onDone={() => {}} onCancel={() => {}} />)
}

/** The SSO connection as the listing describes it: its fields differ, its catalogue entry lags. */
const sso = (): Provider => ({
    ...bedrock(GUIDE),
    id: 'amazon-bedrock-sso',
    fields: ['start_url', 'sso_region', 'sso_profile', 'region', 'role_name'] as ProviderSummary['fields'],
})

const escaped = (text: string) => text.replaceAll('&', '&amp;').replaceAll('"', '&quot;').replaceAll('<', '&lt;').replaceAll('>', '&gt;')

describe('the provider setup guide', () => {
    it('shows the five steps, the exact policy and the four links for a new connection', () => {
        const html = draw(false)
        expect(html).toContain('How to connect')
        for (const step of GUIDE.steps) expect(html).toContain(step.title)
        expect(html).toContain(escaped(POLICY))
        expect(html).toContain('bedrock:InvokeModel')
        expect(html).toContain('bedrock:ListFoundationModels')
        for (const link of GUIDE.links) {
            expect(html).toContain(link.label)
            expect(html).toContain(link.url)
        }
        expect(html).toContain('Access key ID')
        expect(html).toContain('Secret access key')
        expect(html).toContain('Session token (optional)')
        expect(html).toContain('Region')
    })

    it('stays reachable while editing, collapsed rather than removed', () => {
        const html = draw(true)
        expect(html).toContain('How to connect')
        expect(html).toContain('Show')
        // Collapsed: the policy is not drawn until the person expands the guide. The section is
        // outside the save/result notices, so a failed save or test never hides it.
        expect(html).not.toContain(escaped(POLICY))
        expect(html).not.toContain('Saved, but the connection check failed')
    })

    it('renders no guide for a provider without one', () => {
        const html = draw(false, null)
        expect(html).not.toContain('How to connect')
        expect(html).toContain('Region')
    })
})

/** What must never reach a screen (FR-035), on any body this contract or a later one carries. */
const SECRETS = {
    device_code: 'DEVICE-CODE-4c1f',
    access_token: 'ACCESS-TOKEN-9a2b',
    refresh_token: 'REFRESH-TOKEN-77e0',
    id_token: 'ID-TOKEN-5d31',
    client_secret: 'CLIENT-SECRET-0f8e',
    role_credentials: 'ROLE-CREDENTIALS-aa11',
}

/** The pending answer, contract §3, widened with the secrets a wrong body would add. */
const STARTED: SignInStarted = {
    user_code: 'ABCD-EFGH',
    verification_uri: 'https://device.sso.us-east-1.amazonaws.com/',
    verification_uri_complete: 'https://device.sso.us-east-1.amazonaws.com/?user_code=ABCD-EFGH',
    expires_at: '2026-11-02T05:12:00Z',
    interval_seconds: 5,
}
const STARTED_WITH_SECRETS: SignInStarted & typeof SECRETS = { ...STARTED, ...SECRETS }

/** The approved answer, likewise widened: what the panel drops is the point of the test. */
const ANSWER: Extract<SignInResponse, { state: 'signed-in' }> & typeof SECRETS = {
    state: 'signed-in',
    account_id: '123456789012',
    role_name: 'BedrockOperator',
    expires_at: '2026-11-02T13:12:00Z',
    ...SECRETS,
}

const DENIED = 'the sign-in was denied in the browser'

function drawSignIn(state: Partial<SignInPanelState> = {}) {
    const full: SignInPanelState = { phase: 'idle', account: '', role: '', ...state }
    return renderToString(<SignInPanel state={full} onStart={() => {}} onPick={() => {}} />)
}

/** No token, code or secret reaches the HTML, by value or by field name (FR-009b, FR-035). */
function expectNoSecrets(html: string) {
    for (const [field, value] of Object.entries(SECRETS)) {
        expect(html).not.toContain(value)
        expect(html).not.toContain(field)
    }
}

describe('the sign-in panel', () => {
    it('reads a fresh code, still waiting and done apart', () => {
        expect(signInStep(STARTED)).toEqual({ kind: 'started', started: STARTED })
        // Pending now carries its own expiry: the panel has no code, but it can say when the flow ends.
        expect(signInStep({ state: 'pending', expires_at: STARTED.expires_at })).toEqual({
            kind: 'pending',
            expires_at: STARTED.expires_at,
        })
        // The approved answer is narrowed to the three fields the contract may show (FR-035).
        const step = signInStep(ANSWER)
        expect(step.kind).toBe('signed-in')
        if (step.kind !== 'signed-in') return
        expect(step.session).toEqual({
            state: 'signed-in',
            account_id: '123456789012',
            role_name: 'BedrockOperator',
            expires_at: '2026-11-02T13:12:00Z',
        })
    })

    it('pending: shows the code and where to type it, and no token or secret', () => {
        const html = drawSignIn({ phase: 'waiting', started: STARTED_WITH_SECRETS })
        expect(html).toContain('Type this code at')
        expect(html).toContain('ABCD-EFGH')
        expect(html).toContain(STARTED.verification_uri)
        expect(html).toContain(STARTED.verification_uri_complete)
        expect(html).toContain('Waiting for the confirmation')
        // The interval is its own text node; the sentence is asserted in two halves.
        expect(html).toContain('Checking back every')
        expect(html).toContain('seconds.')
        expect(html).toContain('Start again')
        // Nothing of an answer that has not resolved yet.
        expect(html).not.toContain('the session ends')
        expect(html).not.toContain('Sign-in did not complete')
        expectNoSecrets(html)
    })

    it('pending on the way in: names the wait and its deadline instead of calling it a failure', () => {
        const html = drawSignIn({ phase: 'pending', pendingUntil: STARTED.expires_at })
        // The 202 answer is normal (contracts §3): the old "in progress somewhere else" verdict is gone.
        expect(html).not.toContain('already in progress somewhere else')
        expect(html).toContain('Waiting for approval')
        expect(html).toContain('still waiting for approval')
        expect(html).toContain('cannot be shown again')
        expect(html).toContain(absoluteTime(STARTED.expires_at))
        expect(html).toContain('Start again')
        expect(html).toContain('Checking back every')
        // No code is drawn or restored, and nothing failed.
        expect(html).not.toContain('Type this code at')
        expect(html).not.toContain('ABCD-EFGH')
        expect(html).not.toContain('Sign-in did not complete')
        expectNoSecrets(html)
    })

    it('success: states the account, the role and the expiry, and forgets the code', () => {
        const step = signInStep(ANSWER)
        const session = step.kind === 'signed-in' ? step.session : undefined
        const html = drawSignIn({
            phase: 'signed-in',
            session,
            accounts: { accounts: [{ account_id: '123456789012', account_name: 'Acme Prod' }], fallback: false },
            roles: { roles: [{ role_name: 'BedrockOperator', account_id: '123456789012' }], fallback: false },
            account: '123456789012',
            role: 'BedrockOperator',
        })
        expect(html).toContain('Signed in')
        expect(html).toContain('account 123456789012')
        expect(html).toContain('role BedrockOperator')
        expect(html).toContain('the session ends')
        expect(html).toContain('AWS account')
        expect(html).toContain('Sign in again')
        // The waiting code is gone once the sign-in is answered.
        expect(html).not.toContain('ABCD-EFGH')
        expect(html).not.toContain('Sign-in did not complete')
        expectNoSecrets(html)
    })

    it('denied: says so in plain words, with no code, token or secret', () => {
        const html = drawSignIn({ phase: 'failed', failure: DENIED })
        expect(html).toContain('Sign-in did not complete')
        expect(html).toContain(DENIED)
        expect(html).toContain('Sign in')
        expect(html).not.toContain('ABCD-EFGH')
        expect(html).not.toContain('the session ends')
        expectNoSecrets(html)
    })
})
// ---- Asking again against the saved connection (spec 044, FR-008)

describe('the re-sign-in', () => {
    it('sends an empty body, so the server falls back to the saved connection', async () => {
        const call = vi.fn(async (_providerId: string, _request?: SignInRequest) => ({
            state: 'pending',
            expires_at: STARTED.expires_at,
        }))
        const step = await reSignInSignIn('amazon-bedrock-sso', call as unknown as typeof ssoSignIn)
        expect(step).toEqual({ kind: 'pending', expires_at: STARTED.expires_at })
        expect(call).toHaveBeenCalledTimes(1)
        expect(call.mock.calls[0][0]).toBe('amazon-bedrock-sso')
        expect(call.mock.calls[0][1]).toEqual({})
        // Nothing of the saved connection is re-sent, and no profile overrides it either.
        expect(call.mock.calls[0][1]).not.toHaveProperty('start_url')
        expect(call.mock.calls[0][1]).not.toHaveProperty('sso_region')
        expect(call.mock.calls[0][1]).not.toHaveProperty('profile')
    })
})

describe('the sign-in form', () => {
    it('draws the sign-in half only for a provider that asks for a start url and an sso region', () => {
        // The listing's own field union lags the endpoint's (catalogue.ts:22); the server does
        // send these four for the SSO provider (contract section 1).
        const ssoFields = ['start_url', 'sso_region', 'sso_profile', 'region', 'role_name'] as ProviderSummary['fields']
        const sso: Provider = { ...bedrock(GUIDE), id: 'amazon-bedrock-sso', fields: ssoFields }
        const drawn = renderToString(<ConnectForm provider={sso} onDone={() => {}} onCancel={() => {}} />)
        expect(drawn).toContain('aria-label="Sign in"')
        expect(drawn).toContain('Identity Center start URL')
        // The key-based form keeps no sign-in half (constitution II: the fields decide, not the id).
        expect(draw(false)).not.toContain('aria-label="Sign in"')
    })
})

// ---- What the stored session is doing (spec 044, FR-009b, FR-034b, SC-021)

/** Each state's label and the words that tell it apart, as a person reads them on the form. */
const SESSION_TEXT = [
    { state: 'configured', label: 'Not signed in yet', says: 'sign in is still needed' },
    { state: 'signed-in', label: 'Signed in', says: 'Vizier is using it' },
    { state: 'expired', label: 'Session expired', says: 'Sign in again to get a new session' },
    { state: 'unreadable', label: 'Session cannot be read', says: 'sign in again to replace it' },
] as const

describe('the stored session state', () => {
    it('draws each of the four states with its own label and its own sentence', () => {
        for (const { state, label, says } of SESSION_TEXT) {
            const html = renderToString(<SessionState session={{ state, profile: 'team-dev' }} />)
            expect(html).toContain('aria-label="Session"')
            expect(html).toContain(label)
            expect(html).toContain(says)
            expect(html).toContain('team-dev')
        }
    })

    it('names the expiry while signed in, and says nothing about one when it is not', () => {
        const at = '2026-11-03T13:12:00Z'
        const signedIn = renderToString(<SessionState session={{ state: 'signed-in', profile: 'team-dev', expires_at: at }} />)
        expect(signedIn).toContain('The session ends')
        expect(signedIn).toContain(absoluteTime(at))
        const expired = renderToString(<SessionState session={{ state: 'expired', profile: 'team-dev', expires_at: at }} />)
        expect(expired).toContain('Session expired')
        expect(expired).not.toContain('The session ends')
        expect(expired).not.toContain(absoluteTime(at))
    })

    it('reaches the form above the fields, and only when the row carries a session', () => {
        const editing = renderToString(
            <ConnectForm provider={sso()} session={{ state: 'expired', profile: 'team-dev' }} editing onDone={() => {}} onCancel={() => {}} />
        )
        expect(editing).toContain('aria-label="Session"')
        expect(editing).toContain('Session expired')
        expect(editing).toContain('Identity Center start URL')
        // Above the field list, so the state is read before the fields that keep it working.
        expect(editing.indexOf('aria-label="Session"')).toBeLessThan(editing.indexOf('Identity Center start URL'))
        // A connection with no stored session draws no such section at all.
        const fresh = renderToString(<ConnectForm provider={sso()} editing onDone={() => {}} onCancel={() => {}} />)
        expect(fresh).not.toContain('aria-label="Session"')
    })

    it('offers "Sign in again" from the card for expired and unreadable sessions only', () => {
        // Both states have a session to renew; the card is the place it is read.
        for (const state of ['expired', 'unreadable'] as const) {
            const html = renderToString(<SessionState session={{ state, profile: 'team-dev' }} onReSignIn={() => {}} />)
            expect(html).toContain('>Sign in again<')
            // Inside the form: a submit button here would save the credentials instead.
            expect(html).toContain('type="button"')
        }
        // A live session has the panel's own button; a configured one has nothing to renew yet.
        for (const state of ['signed-in', 'configured'] as const) {
            const html = renderToString(<SessionState session={{ state, profile: 'team-dev' }} onReSignIn={() => {}} />)
            expect(html).not.toContain('>Sign in again<')
        }
        // Without a callback the card stays read-only.
        const readOnly = renderToString(<SessionState session={{ state: 'expired', profile: 'team-dev' }} />)
        expect(readOnly).not.toContain('>Sign in again<')
    })

    it('wires the card to a new code in the form, and leaves a signed-in session alone', () => {
        const expired = renderToString(
            <ConnectForm provider={sso()} editing session={{ state: 'expired', profile: 'team-dev' }} onDone={() => {}} onCancel={() => {}} />
        )
        expect(expired).toContain('aria-label="Session"')
        expect(expired).toContain('>Sign in again<')
        const signedIn = renderToString(
            <ConnectForm provider={sso()} editing session={{ state: 'signed-in', profile: 'team-dev' }} onDone={() => {}} onCancel={() => {}} />
        )
        expect(signedIn).toContain('aria-label="Session"')
        expect(signedIn).not.toContain('>Sign in again<')
    })

    it('shows no token, code or key, even when the session object carries one', () => {
        // A wider body than any contract allows, cast in: the section reads four fields and nothing else.
        const poisoned = {
            state: 'signed-in',
            profile: 'team-dev',
            expires_at: '2026-11-03T13:12:00Z',
            token: 'TOKEN-6d1a',
            access_token: 'ACCESS-TOKEN-6d1a',
            refresh_token: 'REFRESH-TOKEN-6d1a',
            authorization_code: 'AUTH-CODE-6d1a',
            device_code: 'DEVICE-CODE-6d1a',
            client_secret: 'CLIENT-SECRET-6d1a',
            secret_access_key: 'SECRET-KEY-6d1a',
        } as unknown as SsoStateView
        const html = renderToString(<ConnectForm provider={sso()} session={poisoned} editing onDone={() => {}} onCancel={() => {}} />)
        expect(html).toContain('Signed in')
        expect(html).toContain('team-dev')
        for (const secret of ['TOKEN-6d1a', 'AUTH-CODE-6d1a', 'DEVICE-CODE-6d1a', 'CLIENT-SECRET-6d1a', 'SECRET-KEY-6d1a']) {
            expect(html).not.toContain(secret)
        }
    })
})
