import { useCallback, useEffect, useState } from 'react'
import { getIntegrationGrants, setIntegrationGrants } from '../services/vizier'
import type { GrantFamily, IntegrationGrants } from '../interfaces/types'

/** What the agent may manage itself (spec 017), stored apart from its config: saving them
 *  never restarts the agent, and only someone who manages its integrations may change them. */
const FAMILIES: { key: GrantFamily; label: string; note?: string; warn?: boolean }[] = [
  {
    key: 'mcp',
    label: 'MCP servers',
    note: 'Includes local servers, which run commands on the vizier host.',
    warn: true,
  },
  { key: 'addon', label: 'Addons' },
  {
    key: 'channel',
    label: 'Channels',
    note: 'Can add, replace tokens on, or delete its Discord/Telegram bots — including the one it is talking on.',
  },
]

type Draft = Record<GrantFamily, boolean>

const fromGrants = (g: IntegrationGrants): Draft => ({
  mcp: g.grants.mcp.on,
  addon: g.grants.addon.on,
  channel: g.grants.channel.on,
})

/** The agent's grants, a draft of them, and a save that sends only what changed. */
export function useSelfManagementGrants(agentId: string | undefined) {
  const [grants, setGrants] = useState<IntegrationGrants | null>(null)
  const [draft, setDraft] = useState<Draft | null>(null)

  useEffect(() => {
    setGrants(null)
    setDraft(null)
    if (!agentId) return
    getIntegrationGrants(agentId)
      .then((res) => {
        setGrants(res.data)
        setDraft(fromGrants(res.data))
      })
      .catch(() => {})
  }, [agentId])

  const changes = useCallback((): Partial<Draft> => {
    if (!grants || !draft) return {}
    const saved = fromGrants(grants)
    return Object.fromEntries(
      (Object.keys(draft) as GrantFamily[]).filter((k) => draft[k] !== saved[k]).map((k) => [k, draft[k]])
    )
  }, [grants, draft])

  const changed = Object.keys(changes()).length > 0

  const save = useCallback(async () => {
    const diff = changes()
    if (!agentId || Object.keys(diff).length === 0) return
    const res = await setIntegrationGrants(agentId, diff)
    setGrants(res.data)
    setDraft(fromGrants(res.data))
  }, [agentId, changes])

  const reset = useCallback(() => {
    if (grants) setDraft(fromGrants(grants))
  }, [grants])

  return { grants, draft, setDraft, changed, save, reset }
}

export default function SelfManagementSection({
  grants,
  draft,
  onChange,
}: {
  grants: IntegrationGrants | null
  draft: Draft | null
  onChange: (draft: Draft) => void
}) {
  if (!grants || !draft) return null
  const editable = grants.can_manage
  return (
    <div>
      <h4
        style={{
          fontSize: '0.85rem',
          fontWeight: 600,
          color: 'var(--text-primary)',
          marginBottom: '0.75rem',
          paddingBottom: '0.5rem',
          borderBottom: '1px solid var(--border)',
        }}
      >
        Self-management
      </h4>
      <div
        style={{
          padding: '0.75rem',
          border: '1px solid var(--border)',
          borderRadius: '0.5rem',
          display: 'flex',
          flexDirection: 'column',
          gap: '0.6rem',
        }}
      >
        <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>
          {editable
            ? 'Let this agent change its own integrations from conversation. Changes take effect from its next turn.'
            : 'Only people who can manage this agent’s integrations can change these.'}
        </div>
        {FAMILIES.map(({ key, label, note, warn }) => (
          <div key={key}>
            <label
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '0.4rem',
                fontSize: '0.8rem',
                cursor: editable ? 'pointer' : 'not-allowed',
              }}
            >
              <input
                type="checkbox"
                checked={draft[key]}
                disabled={!editable}
                onChange={(e) => onChange({ ...draft, [key]: e.target.checked })}
              />
              {label}
              {grants.grants[key].on && grants.grants[key].granted_by && (
                <span style={{ color: 'var(--text-tertiary)', fontSize: '0.72rem' }}>
                  (allowed by {grants.grants[key].granted_by})
                </span>
              )}
            </label>
            {note && (
              <div
                style={{
                  marginLeft: '1.4rem',
                  fontSize: '0.72rem',
                  color: warn ? '#f59e0b' : 'var(--text-tertiary)',
                }}
              >
                {warn ? '⚠ ' : ''}
                {note}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  )
}
