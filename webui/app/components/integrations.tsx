import { useEffect, useState, type CSSProperties, type ReactNode } from 'react'
import { FaTriangleExclamation } from 'react-icons/fa6'
import { integrationIconUrl } from '../services/vizier'
import type { IntegrationItem, IntegrationStatus } from '../interfaces/types'

/** Shared pieces of the Integrations pages (Channels, Addons, MCP). */

export const inputStyle: CSSProperties = {
  width: '100%',
  padding: '0.5rem 0.75rem',
  borderRadius: '0.375rem',
  border: '1px solid var(--border)',
  background: 'var(--surface)',
  color: 'var(--text-primary)',
  fontSize: '0.875rem',
  outline: 'none',
}

export const labelStyle: CSSProperties = {
  display: 'block',
  marginBottom: '0.25rem',
  fontSize: '0.8rem',
  fontWeight: 500,
  color: 'var(--text-secondary)',
}

export const MASK = '••••'

export const isMasked = (value: unknown) => typeof value === 'string' && value.startsWith(MASK)

/** Every Integrations page: a header with the page's actions, then its body. */
export function IntegrationsPage({
  title,
  subtitle,
  actions,
  readOnly,
  children,
}: {
  title: string
  subtitle: string
  actions?: ReactNode
  readOnly?: boolean
  children: ReactNode
}) {
  return (
    <>
      <div className="main-header" style={{ display: 'flex', alignItems: 'center', gap: '1rem' }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <h3 style={{ margin: 0 }}>{title}</h3>
          <div style={{ fontSize: '0.8rem', color: 'var(--text-tertiary)' }}>{subtitle}</div>
        </div>
        {actions}
      </div>
      <div className="main-body" style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
        {readOnly && (
          <div style={noticeStyle}>
            You can see what is attached to this agent, but not change it.
          </div>
        )}
        {children}
      </div>
    </>
  )
}

export const noticeStyle: CSSProperties = {
  padding: '0.6rem 0.8rem',
  borderRadius: '0.5rem',
  border: '1px solid var(--border)',
  background: 'var(--hover)',
  fontSize: '0.8rem',
  color: 'var(--text-secondary)',
}

export function KindIcon({ kind, size = 28 }: { kind: string; size?: number }) {
  return (
    <img
      src={integrationIconUrl(kind)}
      alt=""
      width={size}
      height={size}
      style={{ borderRadius: size / 4, flexShrink: 0 }}
    />
  )
}

const STATUS_LOOK: Record<IntegrationStatus['state'], { label: string; color: string }> = {
  running: { label: 'Running', color: '#10b981' },
  connecting: { label: 'Connecting', color: '#3b82f6' },
  reconnecting: { label: 'Reconnecting', color: '#f59e0b' },
  failed: { label: 'Failed', color: '#ef4444' },
  unreachable: { label: 'Unreachable', color: '#ef4444' },
  incomplete: { label: 'Needs setup', color: '#f59e0b' },
  disabled: { label: 'Disabled', color: '#6b7280' },
  unknown: { label: 'Unknown', color: '#6b7280' },
}

/** The status this agent sees; an integration not attached to it is just "Not in use". */
export function StatusBadge({ item, runningLabel }: { item: IntegrationItem; runningLabel?: string }) {
  const status = item.status
  const look = !item.attached && status.state !== 'disabled'
    ? { label: 'Not in use', color: '#6b7280' }
    : status.state === 'running' && runningLabel
      ? { ...STATUS_LOOK.running, label: runningLabel }
      : STATUS_LOOK[status.state]
  const tools = status.state === 'running' && status.tools != null ? ` · ${status.tools} tools` : ''
  const error = 'last_error' in status ? status.last_error : undefined
  return (
    <span
      title={error}
      style={{ display: 'inline-flex', alignItems: 'center', gap: '0.35rem', fontSize: '0.8rem', color: 'var(--text-secondary)' }}
    >
      <span style={{ width: 8, height: 8, borderRadius: '50%', background: look.color, flexShrink: 0 }} />
      {look.label}
      {tools}
    </span>
  )
}

export function statusError(item: IntegrationItem): string | undefined {
  return 'last_error' in item.status ? item.status.last_error : undefined
}

/** Before saving an integration other agents use too. */
export function SharedEditWarning({ item }: { item: IntegrationItem }) {
  if (item.also_used_by.length === 0 && !item.others) return null
  const names = item.also_used_by.map((a) => a.name)
  if (item.others) names.push(`${item.others} more`)
  return (
    <div style={{ ...noticeStyle, display: 'flex', gap: '0.5rem', alignItems: 'flex-start', borderColor: '#f59e0b' }}>
      <FaTriangleExclamation style={{ color: '#f59e0b', marginTop: 2, flexShrink: 0 }} />
      <span>
        Also used by {names.join(', ')}. Saving applies to them too, from their next turn.
      </span>
    </div>
  )
}

/** A secret setting: the stored value shows redacted with a Replace button, and only a
 *  replaced value is sent (`undefined` means "unchanged"). */
export function SecretInput({
  stored,
  value,
  onChange,
  placeholder,
  disabled,
}: {
  stored?: unknown
  value: string | undefined
  onChange: (value: string | undefined) => void
  placeholder?: string
  disabled?: boolean
}) {
  const hasStored = typeof stored === 'string' && stored !== ''
  const [replacing, setReplacing] = useState(!hasStored)
  // Once a replaced value is saved (the parent clears `value`) or a new one is stored, show the
  // stored, redacted value again rather than an empty input.
  useEffect(() => {
    if (value === undefined) setReplacing(!hasStored)
  }, [value, stored])
  if (!replacing) {
    return (
      <div style={{ display: 'flex', gap: '0.5rem', alignItems: 'center' }}>
        <code style={{ ...inputStyle, flex: 1, fontFamily: 'monospace', color: 'var(--text-tertiary)' }}>
          {String(stored)}
        </code>
        {!disabled && (
          <button type="button" className="btn btn-secondary" style={smallButton} onClick={() => setReplacing(true)}>
            Replace
          </button>
        )}
      </div>
    )
  }
  return (
    <div style={{ display: 'flex', gap: '0.5rem', alignItems: 'center' }}>
      <input
        type="password"
        autoComplete="off"
        style={{ ...inputStyle, flex: 1 }}
        placeholder={placeholder}
        value={value ?? ''}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
      />
      {hasStored && (
        <button
          type="button"
          className="btn btn-ghost"
          style={smallButton}
          onClick={() => {
            setReplacing(false)
            onChange(undefined)
          }}
        >
          Keep
        </button>
      )}
    </div>
  )
}

export const smallButton: CSSProperties = { padding: '6px 12px', fontSize: '0.8rem' }

export function Switch({
  checked,
  onChange,
  disabled,
  title,
}: {
  checked: boolean
  onChange: (next: boolean) => void
  disabled?: boolean
  title?: string
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      title={title}
      disabled={disabled}
      onClick={(e) => {
        e.stopPropagation()
        onChange(!checked)
      }}
      style={{
        width: 36,
        height: 20,
        borderRadius: 10,
        border: 'none',
        padding: 2,
        background: checked ? 'var(--accent-primary)' : 'var(--border)',
        cursor: disabled ? 'not-allowed' : 'pointer',
        opacity: disabled ? 0.5 : 1,
        transition: 'background 0.15s',
        flexShrink: 0,
      }}
    >
      <span
        style={{
          display: 'block',
          width: 16,
          height: 16,
          borderRadius: '50%',
          background: '#fff',
          transform: checked ? 'translateX(16px)' : 'none',
          transition: 'transform 0.15s',
        }}
      />
    </button>
  )
}

/** A destructive action that asks once more, inline, before it runs. */
export function ConfirmButton({
  label,
  confirmLabel,
  prompt,
  onConfirm,
  disabled,
}: {
  label: string
  confirmLabel: string
  prompt?: string
  onConfirm: () => void
  disabled?: boolean
}) {
  const [asking, setAsking] = useState(false)
  if (!asking) {
    return (
      <button type="button" className="btn btn-ghost" style={{ ...smallButton, color: '#ef4444' }} disabled={disabled} onClick={() => setAsking(true)}>
        {label}
      </button>
    )
  }
  return (
    <span style={{ display: 'inline-flex', gap: '0.5rem', alignItems: 'center', fontSize: '0.8rem' }}>
      {prompt && <span style={{ color: 'var(--text-secondary)' }}>{prompt}</span>}
      <button type="button" className="btn btn-ghost" style={smallButton} onClick={() => setAsking(false)}>
        Cancel
      </button>
      <button
        type="button"
        className="btn"
        style={{ ...smallButton, background: '#ef4444', color: '#fff', border: 'none' }}
        onClick={() => {
          setAsking(false)
          onConfirm()
        }}
      >
        {confirmLabel}
      </button>
    </span>
  )
}
