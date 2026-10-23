// Shared look for the provider settings components, matching the agent form.

export const inputStyle: React.CSSProperties = {
  width: '100%',
  padding: '0.5rem 0.75rem',
  borderRadius: '0.375rem',
  border: '1px solid var(--border)',
  background: 'var(--surface)',
  color: 'var(--text-primary)',
  fontSize: '0.875rem',
  outline: 'none',
}

export const labelStyle: React.CSSProperties = {
  display: 'block',
  marginBottom: '0.25rem',
  fontSize: '0.8rem',
  fontWeight: 500,
  color: 'var(--text-secondary)',
}

export const hintStyle: React.CSSProperties = {
  fontSize: '0.75rem',
  color: 'var(--text-tertiary)',
  marginTop: '0.25rem',
}

export const cardStyle: React.CSSProperties = {
  border: '1px solid var(--border)',
  borderRadius: '0.5rem',
  background: 'var(--surface)',
  padding: '1rem',
}

export const primaryButton = (disabled = false): React.CSSProperties => ({
  padding: '0.5rem 1.1rem',
  borderRadius: '0.375rem',
  border: 'none',
  background: disabled ? 'var(--border)' : 'var(--accent-primary)',
  color: '#fff',
  cursor: disabled ? 'not-allowed' : 'pointer',
  fontSize: '0.85rem',
  fontWeight: 500,
})

export const secondaryButton: React.CSSProperties = {
  padding: '0.5rem 1.1rem',
  borderRadius: '0.375rem',
  border: '1px solid var(--border)',
  background: 'transparent',
  color: 'var(--text-primary)',
  cursor: 'pointer',
  fontSize: '0.85rem',
}

export const badgeStyle = (tone: 'neutral' | 'ok' | 'warn' | 'error' = 'neutral'): React.CSSProperties => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: '0.25rem',
  padding: '0.05rem 0.45rem',
  borderRadius: '999px',
  fontSize: '0.7rem',
  fontWeight: 500,
  whiteSpace: 'nowrap',
  alignSelf: 'flex-start',
  border: '1px solid var(--border)',
  color:
    tone === 'ok'
      ? 'var(--success, #16a34a)'
      : tone === 'warn'
        ? 'var(--warning, #d97706)'
        : tone === 'error'
          ? 'var(--error, #ef4444)'
          : 'var(--text-secondary)',
})

/** The `message` of a failed API call. */
export function errorMessage(err: unknown, fallback = 'Something went wrong'): string {
  if (err && typeof err === 'object' && 'response' in err) {
    const resp = (err as { response?: { data?: { message?: string } } }).response
    return resp?.data?.message || fallback
  }
  return fallback
}
