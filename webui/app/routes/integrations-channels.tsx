import { useState } from 'react'
import { useParams } from 'react-router'
import {
  ConfirmButton,
  IntegrationsPage,
  KindIcon,
  SecretInput,
  StatusBadge,
  inputStyle,
  labelStyle,
  smallButton,
  statusError,
} from '../components/integrations'
import { apiError, useIntegrationStore, useIntegrations } from '../hooks/integrationStore'
import { useAgentStore } from '../hooks/agentStore'
import { useToastStore } from '../hooks/toastStore'
import { setIntegrationAttachment } from '../services/vizier'
import type { IntegrationItem, KindListing } from '../interfaces/types'

export default function IntegrationsChannels() {
  const { agentId } = useParams()
  const addToast = useToastStore((s) => s.addToast)
  const agents = useAgentStore((s) => s.agents)
  const store = useIntegrations(agentId, 'channel')
  const { items, kinds, canManage, loading } = store

  const attached = items.filter((i) => i.attached)
  const takenPlatforms = new Set(attached.map((i) => i.kind))
  const freePlatforms = kinds.filter((k) => !takenPlatforms.has(k.id))

  const fail = (err: unknown) => {
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
      {attached.map((item) => (
        <ConnectionCard key={item.id} item={item} agents={agents} currentAgentId={agentId!} onError={fail} />
      ))}
      {attached.length === 0 && !loading && (
        <div style={{ color: 'var(--text-tertiary)', fontSize: '0.85rem' }}>No bot is connected to this agent.</div>
      )}
      {canManage && freePlatforms.length > 0 && (
        <ConnectCard
          platforms={freePlatforms}
          unassigned={items.filter((i) => !i.attached && i.status.state === 'unassigned' && i.can_attach)}
          onError={fail}
        />
      )}
    </IntegrationsPage>
  )
}

function ConnectionCard({
  item,
  agents,
  currentAgentId,
  onError,
}: {
  item: IntegrationItem
  agents: { agent_id: string; name: string }[]
  currentAgentId: string
  onError: (err: unknown) => void
}) {
  const store = useIntegrationStore()
  const kinds = store.kinds
  const addToast = useToastStore((s) => s.addToast)
  const [token, setToken] = useState<string | undefined>(undefined)
  const [moveTo, setMoveTo] = useState('')
  const kind = kinds.find((k) => k.id === item.kind)
  const error = statusError(item)

  const saveToken = async () => {
    try {
      await store.update(item.id, { settings: { ...item.settings, token } })
      setToken(undefined)
      addToast('success', 'Token replaced', 'The bot reconnects with it shortly.')
    } catch (err) {
      onError(err)
    }
  }

  const move = async () => {
    try {
      await setIntegrationAttachment(moveTo, item.id, true)
      await store.refreshStatus()
      addToast('success', `${item.name} moved`, `It now answers as ${agents.find((a) => a.agent_id === moveTo)?.name ?? moveTo}.`)
    } catch (err) {
      onError(err)
    }
  }

  return (
    <div className="card-static" style={{ padding: '1rem 1.25rem', display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
        <KindIcon kind={item.kind} />
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontWeight: 600 }}>{item.name}</div>
          <div style={{ fontSize: '0.75rem', color: 'var(--text-tertiary)' }}>{kind?.name ?? item.kind}</div>
        </div>
        <StatusBadge item={item} runningLabel="Connected" />
      </div>
      {error && (
        <div style={{ padding: '0.5rem 0.75rem', borderRadius: '0.375rem', background: 'rgba(239,68,68,0.08)', color: '#ef4444', fontSize: '0.8rem' }}>
          {error}
        </div>
      )}
      {item.can_edit && (
        <section>
          <label style={labelStyle}>Bot token</label>
          <div style={{ display: 'flex', gap: '0.5rem' }}>
            <div style={{ flex: 1 }}>
              <SecretInput stored={item.settings.token} value={token} onChange={setToken} />
            </div>
            {token && (
              <button className="btn btn-primary" style={smallButton} onClick={saveToken}>Save</button>
            )}
          </div>
        </section>
      )}
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.5rem', alignItems: 'center' }}>
        {item.can_edit && (
          <button
            className="btn btn-secondary"
            style={smallButton}
            onClick={async () => {
              try {
                await store.update(item.id, { enabled: !item.enabled })
              } catch (err) {
                onError(err)
              }
            }}
          >
            {item.enabled ? 'Disable' : 'Enable'}
          </button>
        )}
        {item.can_attach && (
          <span style={{ display: 'inline-flex', gap: '0.5rem', alignItems: 'center' }}>
            <select style={{ ...inputStyle, width: 'auto', padding: '0.35rem 0.5rem' }} value={moveTo} onChange={(e) => setMoveTo(e.target.value)}>
              <option value="">Move to agent…</option>
              {agents.filter((a) => a.agent_id !== currentAgentId).map((a) => (
                <option key={a.agent_id} value={a.agent_id}>{a.name}</option>
              ))}
            </select>
            {moveTo && <button className="btn btn-secondary" style={smallButton} onClick={move}>Move</button>}
          </span>
        )}
        <div style={{ flex: 1 }} />
        {item.can_attach && (
          <button
            className="btn btn-ghost"
            style={smallButton}
            onClick={async () => {
              try {
                await store.detach(item.id)
                addToast('success', `${item.name} unassigned`)
              } catch (err) {
                onError(err)
              }
            }}
          >
            Unassign
          </button>
        )}
        {item.can_edit && (
          <ConfirmButton
            label="Delete"
            confirmLabel="Delete"
            prompt="Delete this connection?"
            onConfirm={async () => {
              try {
                await store.remove(item.id, true)
                addToast('success', `${item.name} deleted`)
              } catch (err) {
                onError(err)
              }
            }}
          />
        )}
      </div>
    </div>
  )
}

function ConnectCard({
  platforms,
  unassigned,
  onError,
}: {
  platforms: KindListing[]
  unassigned: IntegrationItem[]
  onError: (err: unknown) => void
}) {
  const store = useIntegrationStore()
  const addToast = useToastStore((s) => s.addToast)
  const [platform, setPlatform] = useState(platforms[0]?.id ?? '')
  const [name, setName] = useState('')
  const [token, setToken] = useState<string | undefined>(undefined)
  const takeOver = unassigned.filter((i) => i.kind === platform)

  const connect = async () => {
    try {
      await store.create({ kind: platform, name: name.trim() || `${platform} bot`, settings: { token } })
      setName('')
      setToken(undefined)
      addToast('success', 'Bot connected', 'It shows as running once it has logged in.')
    } catch (err) {
      onError(err)
    }
  }

  return (
    <div className="card-static" style={{ padding: '1rem 1.25rem', display: 'flex', flexDirection: 'column', gap: '0.75rem', borderStyle: 'dashed' }}>
      <div style={{ fontWeight: 600 }}>Connect a bot</div>
      <div style={{ display: 'flex', gap: '0.5rem' }}>
        {platforms.map((p) => (
          <button
            key={p.id}
            className={`btn ${platform === p.id ? 'btn-primary' : 'btn-secondary'}`}
            style={smallButton}
            onClick={() => setPlatform(p.id)}
          >
            <KindIcon kind={p.id} size={16} /> {p.name}
          </button>
        ))}
      </div>
      <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap' }}>
        <input style={{ ...inputStyle, maxWidth: 220 }} placeholder="Name" value={name} onChange={(e) => setName(e.target.value)} />
        <div style={{ flex: 1, minWidth: 220 }}>
          <SecretInput value={token} onChange={setToken} placeholder="Bot token" />
        </div>
        <button className="btn btn-primary" style={smallButton} disabled={!token} onClick={connect}>Connect</button>
      </div>
      {takeOver.length > 0 && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: '0.4rem' }}>
          <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>Or take over an unassigned connection:</div>
          {takeOver.map((i) => (
            <div key={i.id} style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', fontSize: '0.85rem' }}>
              <KindIcon kind={i.kind} size={18} />
              <span style={{ flex: 1 }}>{i.name}</span>
              <button
                className="btn btn-secondary"
                style={smallButton}
                onClick={async () => {
                  try {
                    await store.attach(i.id)
                    addToast('success', `${i.name} connected to this agent`)
                  } catch (err) {
                    onError(err)
                  }
                }}
              >
                Use here
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
