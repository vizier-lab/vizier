import { Button, Notice, Select, TextField } from '@vizier/design'
import { useEffect, useState } from 'react'
import type { ProviderGuide, ProviderSummary } from '../../api/endpoints/catalogue'
import {
    saveProvider,
    ssoAccounts,
    ssoRoles,
    ssoSignIn,
    testProvider,
    type AccountList,
    type CredentialField,
    type RoleList,
    type SignInProgress,
    type SignInRequest,
    type SignInResponse,
    type SignInStarted,
    type TestResult,
} from '../../api/endpoints/providers'
import { absoluteTime } from '../../lib/format'
import { FIELD_LABELS, PLAIN_FIELDS } from '../../lib/providers'

type Props = {
    provider: Pick<ProviderSummary, 'id' | 'name' | 'fields' | 'endpoint' | 'doc' | 'guide'>
    /** Editing a connected provider: secrets left empty are kept. */
    editing?: boolean
    onDone: () => void
    onCancel: () => void
    submitLabel?: string
}

/**
 * The provider's own setup steps, rendered from the listing's `guide` rather than from a
 * provider branch. Open for a new connection and collapsed while editing, but always toggleable:
 * a failed save or test must not hide the fix (FR-011a).
 */
function Guide({ guide, editing }: { guide: ProviderGuide; editing?: boolean }) {
    const [open, setOpen] = useState(!editing)
    const [copied, setCopied] = useState(false)

    return (
        <section className="flex flex-col gap-10 rounded-lg border border-border bg-raised p-12" aria-label="How to connect">
            <div className="flex items-center justify-between gap-12">
                <div>
                    <p className="type-ui-medium text-fg">How to connect</p>
                    <p className="type-small text-muted">{guide.intro}</p>
                </div>
                <Button type="button" variant="ghost" aria-expanded={open} onClick={() => setOpen(!open)}>
                    {open ? 'Hide' : 'Show'}
                </Button>
            </div>
            {open && (
                <div className="flex flex-col gap-12">
                    <ol className="flex list-decimal flex-col gap-8 pl-20">
                        {guide.steps.map((step, i) => (
                            <li key={step.title} className="type-small text-muted">
                                <span className="type-small-strong text-fg">
                                    {i + 1}. {step.title}
                                </span>
                                <p>{step.body}</p>
                            </li>
                        ))}
                    </ol>
                    <div className="flex flex-col gap-6">
                        <div className="flex items-center justify-between gap-12">
                            <p className="type-small-strong text-fg">Minimal IAM policy</p>
                            <Button
                                type="button"
                                variant="ghost"
                                onClick={() => {
                                    void navigator.clipboard?.writeText(guide.policy.code)
                                    setCopied(true)
                                }}
                            >
                                {copied ? 'Copied' : 'Copy'}
                            </Button>
                        </div>
                        <pre className="overflow-x-auto rounded-md border border-border bg-ground p-10 type-small text-fg">
                            <code>{guide.policy.code}</code>
                        </pre>
                    </div>
                    <div className="flex flex-wrap gap-x-14 gap-y-6">
                        {guide.links.map((link) => (
                            <a
                                key={link.url}
                                className="type-small text-muted underline"
                                href={link.url}
                                target="_blank"
                                rel="noreferrer"
                            >
                                {link.label}
                            </a>
                        ))}
                    </div>
                </div>
            )}
        </section>
    )
}

/**
 * A provider that signs in rather than taking a key says so with the fields it asks for
 * (spec 044, contracts §1). The form reads that capability and never the provider id
 * (constitution II).
 */
const SIGN_IN_FIELDS: CredentialField[] = ['start_url', 'sso_region']

/** `signed-in` is the one progress answer that carries a resolved session. */
export type SignedIn = Extract<SignInProgress, { state: 'signed-in' }>

/** The panel's phase, from the first click to the answer it settles on. */
export type SignInPhase = 'idle' | 'starting' | 'waiting' | 'signed-in' | 'failed'

/** What one answer to the sign-in call means, read before any panel state is touched. */
export type SignInStep =
    | { kind: 'started'; started: SignInStarted }
    | { kind: 'pending' }
    | { kind: 'signed-in'; session: SignedIn }

/**
 * Reads one answer to the sign-in call: a fresh code, still waiting, or done. The session carries
 * only the three non-secret fields the contract allows, so a body that grew a token could not
 * reach the screen (FR-009b, FR-035).
 */
export function signInStep(answer: SignInResponse): SignInStep {
    if ('user_code' in answer) return { kind: 'started', started: answer }
    if (answer.state === 'signed-in') {
        return {
            kind: 'signed-in',
            session: {
                state: 'signed-in',
                account_id: answer.account_id,
                role_name: answer.role_name,
                expires_at: answer.expires_at,
            },
        }
    }
    return { kind: 'pending' }
}

/**
 * The form's values as a sign-in request: an empty field is omitted, so the server falls back to
 * the saved connection and an expired session asks for a code and nothing else (FR-008).
 */
function signInRequest(values: Partial<Record<CredentialField, string>>): SignInRequest {
    const body: SignInRequest = {}
    const startUrl = (values.start_url ?? '').trim()
    const ssoRegion = (values.sso_region ?? '').trim()
    const profile = (values.sso_profile ?? '').trim()
    if (startUrl) body.start_url = startUrl
    if (ssoRegion) body.sso_region = ssoRegion
    if (profile) body.profile = profile
    return body
}

/**
 * The sign-in half of a connection that uses IAM Identity Center: a code and a URL to confirm in
 * a browser on any machine, then the account and role the person is actually entitled to
 * (FR-007, FR-012). Only the code, the URL, the account, the role and the expiry are ever shown —
 * no token, no device code, no secret half (FR-009b, FR-035).
 */
function SignIn({
    providerId,
    values,
    onPicked,
}: {
    providerId: string
    values: Partial<Record<CredentialField, string>>
    /** The chosen pair, kept in the form so saving stores it with the connection (FR-007). */
    onPicked: (accountId: string, roleName: string) => void
}) {
    const [phase, setPhase] = useState<'idle' | 'starting' | 'waiting' | 'signed-in' | 'failed'>('idle')
    /** The request this attempt was started with: typed later, the form must not change it. */
    const [sent, setSent] = useState<SignInRequest>()
    const [started, setStarted] = useState<SignInStarted>()
    const [session, setSession] = useState<SignedIn>()
    const [failure, setFailure] = useState<string>()
    const [accounts, setAccounts] = useState<AccountList>()
    const [roles, setRoles] = useState<RoleList>()
    const [account, setAccount] = useState('')
    const [role, setRole] = useState('')

    function pick(nextAccount: string, nextRole: string) {
        setAccount(nextAccount)
        setRole(nextRole)
        onPicked(nextAccount, nextRole)
    }

    function failed(message: string) {
        setFailure(message)
        setStarted(undefined)
        setPhase('failed')
    }

    function signedIn(answer: SignedIn) {
        setSession(answer)
        setStarted(undefined)
        setPhase('signed-in')
        // What the sign-in resolved is the default pair; the person may still pick another.
        if (answer.account_id || answer.role_name) pick(answer.account_id ?? '', answer.role_name ?? '')
    }

    async function start() {
        setFailure(undefined)
        setSession(undefined)
        setStarted(undefined)
        setAccounts(undefined)
        setRoles(undefined)
        setAccount('')
        setRole('')
        onPicked('', '')
        setPhase('starting')
        const request = signInRequest(values)
        setSent(request)
        try {
            const step = signInStep(await ssoSignIn(providerId, request))
            if (step.kind === 'started') {
                setStarted(step.started)
                setPhase('waiting')
                return
            }
            if (step.kind === 'signed-in') {
                signedIn(step.session)
                return
            }
            failed('The sign-in is already in progress somewhere else. Start again.')
        } catch (err) {
            failed(err instanceof Error ? err.message : 'The sign-in could not be started.')
        }
    }

    // Poll the same call until it is approved, denied or expires (contracts §3: 202 = pending).
    useEffect(() => {
        if (!started || !sent) return
        let cancelled = false
        const interval = Math.max(started.interval_seconds, 5) * 1000
        const deadline = Date.parse(started.expires_at)
        const poll = async () => {
            await new Promise((resolve) => setTimeout(resolve, interval))
            if (cancelled) return
            if (Date.now() > deadline) {
                failed('The sign-in code expired. Start again.')
                return
            }
            try {
                const step = signInStep(await ssoSignIn(providerId, sent))
                if (cancelled) return
                if (step.kind === 'started') {
                    // A new code is never what a poll should return; stop rather than loop on it.
                    failed('The sign-in restarted. Confirm the new code.')
                    return
                }
                if (step.kind === 'signed-in') {
                    signedIn(step.session)
                    return
                }
                void poll()
            } catch (err) {
                if (cancelled) return
                failed(err instanceof Error ? err.message : 'The sign-in could not be completed.')
            }
        }
        void poll()
        return () => {
            cancelled = true
        }
    }, [started, sent, providerId])

    // After a sign-in: the accounts it is assigned, then the roles granted in the chosen account.
    useEffect(() => {
        if (phase !== 'signed-in' || !sent) return
        let cancelled = false
        ssoAccounts(providerId, sent)
            .then((list) => {
                if (cancelled) return
                setAccounts(list)
                if (!account && list.accounts.length === 1) pick(list.accounts[0].account_id, '')
            })
            .catch(() => {
                // The pickers are a convenience: a listing failure leaves the free-text fields.
                if (!cancelled) setAccounts({ accounts: [], fallback: true })
            })
        return () => {
            cancelled = true
        }
    }, [phase, sent, providerId])

    useEffect(() => {
        setRoles(undefined)
        if (!account || !sent) return
        let cancelled = false
        ssoRoles(providerId, account, sent)
            .then((list) => {
                if (cancelled) return
                setRoles(list)
                if (!role && list.roles.length === 1) pick(account, list.roles[0].role_name)
            })
            .catch(() => {
                if (!cancelled) setRoles({ roles: [], fallback: true })
            })
        return () => {
            cancelled = true
        }
    }, [account, sent, providerId])

    return (
        <SignInPanel
            state={{ phase, started, session, failure, accounts, roles, account, role }}
            onStart={() => void start()}
            onPick={pick}
        />
    )
}

/** Everything the panel draws, and nothing the sign-in call returns besides (FR-035). */
export type SignInPanelState = {
    phase: SignInPhase
    started?: SignInStarted
    session?: SignedIn
    failure?: string
    accounts?: AccountList
    roles?: RoleList
    account: string
    role: string
}

/**
 * The panel itself, drawn from a state rather than from effects: waiting, signed in, denied and
 * the pickers of an answered sign-in. Every state the flow can reach is drawable on its own, so a
 * test can read exactly what reaches the screen (FR-009b, FR-035).
 */
export function SignInPanel({
    state,
    onStart,
    onPick,
}: {
    state: SignInPanelState
    onStart: () => void
    onPick: (accountId: string, roleName: string) => void
}) {
    const { phase, started, session, failure, accounts, roles, account, role } = state
    const listedNothing = phase === 'signed-in' && accounts && (accounts.fallback || accounts.accounts.length === 0)
    const noRoles = phase === 'signed-in' && account !== '' && roles && (roles.fallback || roles.roles.length === 0)

    return (
        <section className="flex flex-col gap-10 rounded-lg border border-border bg-raised p-12" aria-label="Sign in">
            <div className="flex items-center justify-between gap-12">
                <div>
                    <p className="type-ui-medium text-fg">Sign in</p>
                    <p className="type-small text-muted">
                        Vizier shows a short code. Confirm it in a browser on any machine, then choose the account
                        and role to use.
                    </p>
                </div>
                <Button type="button" variant="ghost" loading={phase === 'starting'} onClick={onStart}>
                    {phase === 'waiting' ? 'Start again' : session ? 'Sign in again' : 'Sign in'}
                </Button>
            </div>

            {started && (
                <div className="flex flex-col gap-8 rounded-md border border-border bg-ground p-12">
                    <p className="type-small text-muted">
                        Type this code at{' '}
                        <a
                            className="text-fg underline"
                            href={started.verification_uri_complete ?? started.verification_uri}
                            target="_blank"
                            rel="noreferrer"
                        >
                            {started.verification_uri}
                        </a>
                    </p>
                    <p className="type-mono-sm-medium text-fg" aria-label="Confirmation code">
                        {started.user_code}
                    </p>
                    <p className="type-small text-muted" aria-live="polite">
                        Waiting for the confirmation… Checking back every {started.interval_seconds} seconds.
                    </p>
                </div>
            )}

            {session && (
                <Notice tone="info" title="Signed in">
                    {[
                        session.account_id ? `account ${session.account_id}` : undefined,
                        session.role_name ? `role ${session.role_name}` : undefined,
                        `the session ends ${absoluteTime(session.expires_at)}`,
                    ]
                        .filter(Boolean)
                        .join(' · ')}
                </Notice>
            )}

            {phase === 'signed-in' && accounts && accounts.accounts.length > 0 && (
                <div className="flex flex-col gap-10">
                    <Select
                        label="AWS account"
                        value={account || undefined}
                        placeholder="Choose the account"
                        options={accounts.accounts.map((a) => ({
                            value: a.account_id,
                            label: a.account_name ? `${a.account_name} · ${a.account_id}` : a.account_id,
                        }))}
                        onChange={(next) => onPick(next, '')}
                    />
                    <Select
                        label="Role"
                        value={role || undefined}
                        placeholder={roles && roles.roles.length > 0 ? 'Choose the role' : 'Choose an account first'}
                        disabled={!roles || roles.roles.length === 0}
                        options={(roles?.roles ?? []).map((r) => ({ value: r.role_name, label: r.role_name }))}
                        onChange={(next) => onPick(account, next)}
                    />
                </div>
            )}

            {listedNothing && (
                <Notice tone="info" title="Type the account and role">
                    The portal listed no accounts for this identity, so type the account id and the role name in the
                    fields above.
                </Notice>
            )}

            {noRoles && (
                <Notice tone="info" title="Type the role">
                    The portal listed no roles for this account, so type the role name in the field above.
                </Notice>
            )}

            {failure && (
                <Notice tone="failed" title="Sign-in did not complete">
                    {failure}
                </Notice>
            )}
        </section>
    )
}

/**
 * Connect one provider with the fields it takes (a key, an address, a token). Saving also checks
 * the connection, and says what came of it: the credentials are kept either way.
 */
export function ConnectForm({ provider, editing, onDone, onCancel, submitLabel }: Props) {
    const fields: CredentialField[] = provider.fields.length ? provider.fields : ['api_key']
    const [values, setValues] = useState<Partial<Record<CredentialField, string>>>(() =>
        Object.fromEntries(fields.filter((f) => f === 'endpoint' || f === 'base_url').map((f) => [f, provider.endpoint ?? ''])) as Partial<Record<CredentialField, string>>
    )
    const [busy, setBusy] = useState(false)
    const [error, setError] = useState<string>()
    const [result, setResult] = useState<TestResult>()

    const signsIn = SIGN_IN_FIELDS.every((f) => fields.includes(f))
    const ready = fields.some((f) => (values[f] ?? '').trim() !== '') || editing

    return (
        <form
            className="flex flex-col gap-16"
            onSubmit={async (e) => {
                e.preventDefault()
                setBusy(true)
                setError(undefined)
                setResult(undefined)
                try {
                    const body = Object.fromEntries(fields.filter((f) => (values[f] ?? '').trim() !== '').map((f) => [f, (values[f] ?? '').trim()]))
                    await saveProvider(provider.id, body)
                    const test = await testProvider(provider.id).catch(() => undefined)
                    if (test && !test.ok) {
                        setResult(test)
                        return
                    }
                    onDone()
                } catch (err) {
                    setError(err instanceof Error ? err.message : 'It could not be saved.')
                } finally {
                    setBusy(false)
                }
            }}
        >
            {provider.guide && <Guide guide={provider.guide} editing={editing} />}
            {fields.map((f, i) => (
                <TextField
                    key={f}
                    label={FIELD_LABELS[f]}
                    mono
                    type={PLAIN_FIELDS.includes(f) ? 'text' : 'password'}
                    autoComplete="off"
                    autoFocus={i === 0}
                    value={values[f] ?? ''}
                    placeholder={editing && !PLAIN_FIELDS.includes(f) ? 'Leave empty to keep the saved one' : undefined}
                    onChange={(e) => setValues((v) => ({ ...v, [f]: e.target.value }))}
                />
            ))}
            {signsIn && (
                <SignIn
                    providerId={provider.id}
                    values={values}
                    onPicked={(accountId, roleName) =>
                        setValues((v) => ({ ...v, account_id: accountId, role_name: roleName }))
                    }
                />
            )}
            {provider.doc && (
                <a className="type-small text-muted underline" href={provider.doc} target="_blank" rel="noreferrer">
                    {provider.name} documentation
                </a>
            )}
            {error && <Notice tone="failed" title="It could not be saved">{error}</Notice>}
            {result && !result.ok && (
                <Notice
                    tone="retrying"
                    title="Saved, but the connection check failed"
                    actions={<Button onClick={onDone}>Keep it anyway</Button>}
                >
                    {result.error ?? `The provider answered ${result.status ?? 'with an error'}.`}
                </Notice>
            )}
            <div className="flex justify-end gap-8">
                <Button type="button" variant="ghost" onClick={onCancel}>
                    Cancel
                </Button>
                <Button type="submit" variant="primary" loading={busy} disabled={!ready}>
                    {submitLabel ?? (editing ? 'Save' : 'Connect')}
                </Button>
            </div>
        </form>
    )
}
