import { useState, type CSSProperties } from 'react'
import { useParams } from 'react-router'
import {
  ConfirmButton,
  IntegrationsPage,
  KindIcon,
  SecretInput,
  StatusBadge,
  smallButton,
  statusError,
} from '../components/integrations'
import { apiError, useIntegrationStore, useIntegrations } from '../hooks/integrationStore'
import { useToastStore } from '../hooks/toastStore'
import type { IntegrationItem, KindListing } from '../interfaces/types'

/** One row per platform: an agent has at most one bot per platform, so every platform is
 *  always shown, connected or not. */
export default function IntegrationsChannels() {
  const { agentId } = useParams()
  const addToast = useToastStore((s) => s.addToast)
  const { items, kinds, canManage, loading } = useIntegrations(agentId, 'channel')

  const onError = (err: unknown) => {
    const { message, data } = apiError(err)
    const hint =
      data?.error === 'token_in_use'
        ? 'Each bot token can back one connection.'
        : data?.error === 'platform_in_use_on_agent'
          ? 'An agent has one connection per platform.'
          : undefined
    addToast('error', message, hint)
  }

  return (
    <IntegrationsPage
      title="Channels"
      subtitle="Discord and Telegram bots this agent answers. A bot reconnects on its own; changing one never restarts the agent."
      readOnly={!canManage}
    >
      <div className="card-static" style={{ padding: 0 }}>
        {kinds.map((kind, i) => {
          const connected = items.find((item) => item.kind === kind.id && item.attached)
          return (
            <div key={kind.id} style={{ ...rowStyle, borderTop: i === 0 ? 'none' : '1px solid var(--border)' }}>
              <div style={platformStyle}>
                <KindIcon kind={kind.id} />
                <span style={{ fontWeight: 600 }}>{kind.name}</span>
              </div>
              <div style={{ flex: 1, minWidth: 0 }}>
                {connected ? (
                  <ConnectedRow item={connected} onError={onError} />
                ) : (
                  <EmptyRow
                    kind={kind}
                    canManage={canManage}
                    onError={onError}
                  />
                )}
              </div>
            </div>
          )
        })}
        {kinds.length === 0 && (
          <div style={{ ...rowStyle, color: 'var(--text-tertiary)', fontSize: '0.85rem' }}>
            {loading ? 'Loading…' : 'No channel platforms available.'}
          </div>
        )}
      </div>
    </IntegrationsPage>
  )
}

const rowStyle: CSSProperties = {
  display: 'flex',
  gap: '1rem',
  padding: '1rem 1.25rem',
  alignItems: 'flex-start',
  flexWrap: 'wrap',
}

const platformStyle: CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  gap: '0.6rem',
  width: 150,
  flexShrink: 0,
  minHeight: 32,
}

const lineStyle: CSSProperties = { display: 'flex', gap: '0.5rem', alignItems: 'center', flexWrap: 'wrap' }

function ConnectedRow({
  item,
  onError,
}: {
  item: IntegrationItem
  onError: (err: unknown) => void
}) {
  const store = useIntegrationStore()
  const addToast = useToastStore((s) => s.addToast)
  const [token, setToken] = useState<string | undefined>(undefined)
  const error = statusError(item)

  const act = async (what: () => Promise<void>, done?: string, description?: string) => {
    try {
      await what()
      if (done) addToast('success', done, description)
    } catch (err) {
      onError(err)
    }
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.6rem' }}>
      <div style={lineStyle}>
        <span style={{ fontWeight: 500 }}>{item.name}</span>
        <StatusBadge item={item} runningLabel="Connected" />
      </div>
      {error && (
        <div style={{ padding: '0.4rem 0.7rem', borderRadius: '0.375rem', background: 'rgba(239,68,68,0.08)', color: '#ef4444', fontSize: '0.8rem' }}>
          {error}
        </div>
      )}
      {item.can_edit && (
        <div style={lineStyle}>
          <span style={{ fontSize: '0.8rem', color: 'var(--text-secondary)', width: 44 }}>Token</span>
          <div style={{ flex: 1, minWidth: 200 }}>
            <SecretInput stored={item.settings.token} value={token} onChange={setToken} />
          </div>
          {token && (
            <button
              className="btn btn-primary"
              style={smallButton}
              onClick={() =>
                act(async () => {
                  await store.update(item.id, { settings: { ...item.settings, token } })
                  setToken(undefined)
                }, 'Token replaced', 'The bot reconnects with it shortly.')
              }
            >
              Save
            </button>
          )}
        </div>
      )}
      <div style={lineStyle}>
        {item.can_edit && (
          <button className="btn btn-secondary" style={smallButton}
            onClick={() => act(() => store.update(item.id, { enabled: !item.enabled }))}>
            {item.enabled ? 'Disable' : 'Enable'}
          </button>
        )}
        {item.can_edit && (
          <ConfirmButton
            label="Delete"
            confirmLabel="Delete"
            prompt="Delete this connection?"
            onConfirm={() => act(() => store.remove(item.id, true), `${item.name} deleted`)}
          />
        )}
      </div>
    </div>
  )
}

function EmptyRow({
  kind,
  canManage,
  onError,
}: {
  kind: KindListing
  canManage: boolean
  onError: (err: unknown) => void
}) {
  const store = useIntegrationStore()
  const addToast = useToastStore((s) => s.addToast)
  const [token, setToken] = useState<string | undefined>(undefined)

  const act = async (what: () => Promise<unknown>, done: string, description?: string) => {
    try {
      await what()
      addToast('success', done, description)
    } catch (err) {
      onError(err)
    }
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.6rem' }}>
      <div style={{ ...lineStyle, minHeight: 32 }}>
        <span style={{ fontSize: '0.85rem', color: 'var(--text-tertiary)' }}>Not connected</span>
      </div>
      {canManage && (
        <div style={lineStyle}>
          <div style={{ flex: 1, minWidth: 200 }}>
            <SecretInput value={token} onChange={setToken} placeholder={`${kind.name} bot token`} />
          </div>
          <button
            className="btn btn-primary"
            style={smallButton}
            disabled={!token}
            onClick={() =>
              act(async () => {
                await store.create({ kind: kind.id, name: `${kind.name} bot`, settings: { token } })
                setToken(undefined)
              }, `${kind.name} bot connected`, 'It shows as connected once it has logged in.')
            }
          >
            Connect
          </button>
        </div>
      )}
    </div>
  )
}
