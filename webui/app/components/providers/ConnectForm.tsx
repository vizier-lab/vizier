import { useState } from 'react'
import { testProvider, upsertProvider } from '../../services/vizier'
import type {
  CatalogueProvider,
  CredentialField,
  ProviderTestResult,
  UpsertProviderRequest,
} from '../../interfaces/types'
import ProviderLogo from '../ProviderLogo'
import {
  badgeStyle,
  errorMessage,
  hintStyle,
  inputStyle,
  labelStyle,
  primaryButton,
  secondaryButton,
} from './styles'

const FIELD_LABELS: Record<CredentialField, { label: string; secret: boolean; placeholder?: string }> = {
  api_key: { label: 'API key', secret: true },
  endpoint: { label: 'Endpoint', secret: false, placeholder: 'https://<resource>.openai.azure.com' },
  base_url: { label: 'Base URL', secret: false, placeholder: 'http://localhost:11434' },
  access_token: { label: 'Access token', secret: true },
  account_id: { label: 'Account id', secret: false },
}

interface ConnectFormProps {
  provider: CatalogueProvider
  /** Editing saved credentials: secrets show as "keep" until replaced. */
  existing?: { has_api_key: boolean; has_access_token: boolean; endpoint?: string | null; base_url?: string | null; account_id?: string | null }
  onSaved: () => void
  onCancel?: () => void
  compact?: boolean
}

/** Asks for exactly the credentials a provider uses, with Test and Save. */
export default function ConnectForm({ provider, existing, onSaved, onCancel, compact }: ConnectFormProps) {
  const [values, setValues] = useState<UpsertProviderRequest>({
    endpoint: existing?.endpoint ?? undefined,
    base_url: existing?.base_url ?? undefined,
    account_id: existing?.account_id ?? undefined,
  })
  const [testing, setTesting] = useState(false)
  const [saving, setSaving] = useState(false)
  const [result, setResult] = useState<ProviderTestResult | null>(null)
  const [error, setError] = useState<string | null>(null)
  const fields = provider.fields.length ? provider.fields : (['api_key'] as CredentialField[])
  const secretEnv = provider.env.filter((e) => /KEY|TOKEN/.test(e))

  // Only fields with a value are sent, so an untouched secret keeps its stored value.
  const body = (): UpsertProviderRequest => {
    const out: UpsertProviderRequest = {}
    for (const f of fields) {
      const v = values[f]
      if (v !== undefined && v !== '') out[f] = v.trim()
    }
    return out
  }

  const keeps = (f: CredentialField) =>
    (f === 'api_key' && existing?.has_api_key) || (f === 'access_token' && existing?.has_access_token)

  const runTest = async () => {
    setTesting(true)
    setResult(null)
    setError(null)
    try {
      const res = await testProvider(provider.id, body())
      setResult(res.data)
    } catch (err) {
      setError(errorMessage(err, 'Test failed'))
    } finally {
      setTesting(false)
    }
  }

  const save = async () => {
    setSaving(true)
    setError(null)
    try {
      await upsertProvider(provider.id, body())
      onSaved()
    } catch (err) {
      setError(errorMessage(err, 'Could not save'))
    } finally {
      setSaving(false)
    }
  }

  const missingRequired = fields.some(
    (f) => (f === 'api_key' || f === 'access_token') && !keeps(f) && !values[f]?.trim()
  ) && provider.kind !== 'local'

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
      {!compact && (
        <div style={{ display: 'flex', alignItems: 'center', gap: '0.6rem' }}>
          <ProviderLogo id={provider.id} name={provider.name} src={provider.logo} fullColor={provider.logo_full_color} size={28} />
          <div>
            <div style={{ fontWeight: 600, fontSize: '0.95rem' }}>Connect {provider.name}</div>
            <div style={{ fontSize: '0.75rem', color: 'var(--text-tertiary)', fontFamily: 'var(--font-mono)' }}>{provider.id}</div>
          </div>
        </div>
      )}

      {fields.map((f) => {
        const meta = FIELD_LABELS[f]
        return (
          <section key={f}>
            <label style={labelStyle}>{meta.label}</label>
            <input
              style={inputStyle}
              type={meta.secret ? 'password' : 'text'}
              autoComplete="off"
              placeholder={keeps(f) ? '•••••••• (saved — type to replace)' : meta.placeholder}
              value={values[f] ?? ''}
              onChange={(e) => setValues((v) => ({ ...v, [f]: e.target.value }))}
            />
          </section>
        )
      })}

      {secretEnv.length > 0 && (
        <div style={hintStyle}>
          Or set <code>{secretEnv[0]}</code> in the server's environment instead of saving a key here.
        </div>
      )}
      {provider.endpoint && (
        <div style={hintStyle}>
          Requests go to <code>{provider.endpoint}</code>
          {provider.doc && (
            <>
              {' · '}
              <a href={provider.doc} target="_blank" rel="noreferrer" style={{ color: 'var(--accent-primary)' }}>
                docs
              </a>
            </>
          )}
        </div>
      )}

      {result && (
        <div style={badgeStyle(result.ok ? 'ok' : 'error')}>
          {result.ok
            ? `✓ Connected${result.model_count != null ? ` · ${result.model_count} models` : ''}`
            : `✗ ${result.status ? `${result.status} ` : ''}${result.error || 'Failed'}`}
        </div>
      )}
      {error && <div style={{ ...hintStyle, color: 'var(--error, #ef4444)' }}>{error}</div>}

      <div style={{ display: 'flex', gap: '0.5rem', justifyContent: 'flex-end' }}>
        {onCancel && (
          <button type="button" style={secondaryButton} onClick={onCancel}>
            Cancel
          </button>
        )}
        <button type="button" style={secondaryButton} onClick={runTest} disabled={testing}>
          {testing ? 'Testing…' : 'Test'}
        </button>
        <button type="button" style={primaryButton(saving || missingRequired)} onClick={save} disabled={saving || missingRequired}>
          {saving ? 'Saving…' : 'Save'}
        </button>
      </div>
    </div>
  )
}
