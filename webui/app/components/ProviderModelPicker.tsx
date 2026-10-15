import { useEffect, useMemo, useRef, useState } from 'react'
import { FaChevronDown, FaWrench, FaEye, FaBrain, FaMagnifyingGlass } from 'react-icons/fa6'
import ProviderLogo from './ProviderLogo'
import ConnectForm from './providers/ConnectForm'
import {
  formatPrice,
  formatTokens,
  useCatalogueModels,
  useCatalogueProviders,
  useCatalogueStore,
  useModelInfo,
} from '../hooks/catalogueStore'
import { hasPermission } from '../utils/auth'
import type { CatalogueModel, CatalogueProvider } from '../interfaces/types'

export interface ProviderModelValue {
  provider: string
  model: string
}

export interface PickerFilter {
  /** Hide models the catalogue says can't call tools (dream model). */
  toolCallOnly?: boolean
  /** Hide models the catalogue says can't see images (vision model). */
  imageInputOnly?: boolean
}

interface ProviderModelPickerProps {
  value: ProviderModelValue
  onChange: (value: ProviderModelValue) => void
  filter?: PickerFilter
  /** Choosing a provider with no model picks its default model (FR-031). */
  preselectDefault?: boolean
  /** Show the selected model's card under the selectors. */
  showCard?: boolean
  invalid?: boolean
}

const ROW_HEIGHT = 46
const LIST_HEIGHT = 320

const triggerStyle = (invalid?: boolean): React.CSSProperties => ({
  width: '100%',
  display: 'flex',
  alignItems: 'center',
  gap: '0.5rem',
  padding: '0.45rem 0.65rem',
  borderRadius: '0.375rem',
  border: `1px solid ${invalid ? 'var(--error, #ef4444)' : 'var(--border)'}`,
  background: 'var(--surface)',
  color: 'var(--text-primary)',
  fontSize: '0.875rem',
  cursor: 'pointer',
  textAlign: 'left',
  minHeight: '2.4rem',
})

const popoverStyle: React.CSSProperties = {
  position: 'absolute',
  top: 'calc(100% + 4px)',
  left: 0,
  right: 0,
  minWidth: '320px',
  zIndex: 1000,
  background: 'var(--surface)',
  border: '1px solid var(--border)',
  borderRadius: '0.5rem',
  boxShadow: '0 8px 24px rgba(0,0,0,0.18)',
  overflow: 'hidden',
}

const groupStyle: React.CSSProperties = {
  padding: '0.45rem 0.75rem 0.25rem',
  fontSize: '0.68rem',
  fontWeight: 600,
  letterSpacing: '0.04em',
  color: 'var(--text-tertiary)',
}

/** Closes when a click lands outside `ref`. */
function useOutsideClick(ref: React.RefObject<HTMLElement | null>, onOutside: () => void) {
  useEffect(() => {
    const handle = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onOutside()
    }
    document.addEventListener('mousedown', handle)
    return () => document.removeEventListener('mousedown', handle)
  }, [ref, onOutside])
}

function SearchBox({ value, onChange, placeholder }: { value: string; onChange: (v: string) => void; placeholder: string }) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: '0.4rem', padding: '0.5rem 0.65rem', borderBottom: '1px solid var(--border)' }}>
      <FaMagnifyingGlass size={12} style={{ opacity: 0.5 }} />
      <input
        autoFocus
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        style={{ flex: 1, border: 'none', outline: 'none', background: 'transparent', color: 'var(--text-primary)', fontSize: '0.85rem' }}
      />
    </div>
  )
}

/** 🔧 👁 💭, dimmed when unknown, struck through when the catalogue says no. */
export function CapabilityIcons({ caps }: { caps: CatalogueModel['caps'] }) {
  const item = (on: boolean | null, Icon: typeof FaWrench, label: string) => (
    <span
      title={`${label}: ${on === null ? 'unknown' : on ? 'yes' : 'no'}`}
      style={{
        opacity: on === null ? 0.3 : on ? 0.9 : 0.25,
        textDecoration: on === false ? 'line-through' : undefined,
        display: 'inline-flex',
      }}
    >
      <Icon size={11} />
    </span>
  )
  return (
    <span style={{ display: 'inline-flex', gap: '0.35rem', alignItems: 'center', color: 'var(--text-secondary)' }}>
      {item(caps.tool_call, FaWrench, 'Tool calling')}
      {item(caps.image_input, FaEye, 'Image input')}
      {item(caps.reasoning, FaBrain, 'Reasoning')}
    </span>
  )
}

/** One model in a list: lab logo, name, context, capabilities, price. */
export function ModelRow({ model, selected, onClick }: { model: CatalogueModel; selected?: boolean; onClick?: () => void }) {
  const price = formatPrice(model.cost)
  return (
    <div
      onClick={onClick}
      role={onClick ? 'option' : undefined}
      aria-selected={selected}
      style={{
        height: ROW_HEIGHT,
        display: 'flex',
        alignItems: 'center',
        gap: '0.6rem',
        padding: '0 0.75rem',
        cursor: onClick ? 'pointer' : 'default',
        background: selected ? 'var(--hover)' : undefined,
        borderBottom: '1px solid var(--border)',
      }}
      onMouseEnter={(e) => onClick && (e.currentTarget.style.background = 'var(--hover)')}
      onMouseLeave={(e) => onClick && (e.currentTarget.style.background = selected ? 'var(--hover)' : '')}
    >
      <ProviderLogo id={model.id} name={model.name} src={model.logo} fullColor={model.logo_full_color} size={18} />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: '0.83rem', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
          {model.name}
          {model.status === 'deprecated' && <span style={{ marginLeft: 6, fontSize: '0.65rem', color: 'var(--warning, #d97706)' }}>deprecated</span>}
          {(model.status === 'beta' || model.status === 'alpha') && <span style={{ marginLeft: 6, fontSize: '0.65rem', color: 'var(--text-tertiary)' }}>{model.status}</span>}
          {model.origin === 'server' && <span style={{ marginLeft: 6, fontSize: '0.65rem', color: 'var(--text-tertiary)' }}>from server</span>}
        </div>
        <div style={{ fontSize: '0.7rem', color: 'var(--text-tertiary)', fontFamily: 'var(--font-mono)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
          {model.id}
        </div>
      </div>
      <span style={{ fontSize: '0.72rem', color: 'var(--text-secondary)', minWidth: '2.6rem', textAlign: 'right' }}>
        {formatTokens(model.limit.input ?? model.limit.context)}
      </span>
      <CapabilityIcons caps={model.caps} />
      <span style={{ fontSize: '0.7rem', color: 'var(--text-tertiary)', minWidth: '5rem', textAlign: 'right' }}>{price ?? ''}</span>
    </div>
  )
}

/** Only the rows in view are rendered, so OpenRouter's hundreds of models filter instantly. */
export function VirtualList<T>({ items, render, height = LIST_HEIGHT }: { items: T[]; render: (item: T, index: number) => React.ReactNode; height?: number }) {
  const [scrollTop, setScrollTop] = useState(0)
  const start = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - 4)
  const end = Math.min(items.length, Math.ceil((scrollTop + height) / ROW_HEIGHT) + 4)
  return (
    <div style={{ maxHeight: height, overflowY: 'auto' }} onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}>
      <div style={{ height: items.length * ROW_HEIGHT, position: 'relative' }}>
        <div style={{ position: 'absolute', top: start * ROW_HEIGHT, left: 0, right: 0 }}>
          {items.slice(start, end).map((item, i) => render(item, start + i))}
        </div>
      </div>
    </div>
  )
}

/** The selected model's limits, capabilities, price and knowledge cutoff. */
export function ModelCard({ model, checkpointThreshold }: { model: CatalogueModel; checkpointThreshold?: number }) {
  const context = model.limit.input ?? model.limit.context
  const capLabel = (v: boolean | null) => (v === null ? 'unknown' : v ? 'yes' : 'no')
  const row = (label: string, value: React.ReactNode) => (
    <div style={{ display: 'flex', justifyContent: 'space-between', gap: '1rem', fontSize: '0.78rem' }}>
      <span style={{ color: 'var(--text-tertiary)' }}>{label}</span>
      <span style={{ color: 'var(--text-primary)', textAlign: 'right' }}>{value}</span>
    </div>
  )
  return (
    <div style={{ border: '1px solid var(--border)', borderRadius: '0.5rem', padding: '0.75rem', display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: '0.35rem 1.5rem' }}>
      {row('Context window', context ? `${context.toLocaleString()} tokens` : 'unknown')}
      {row('Max output', model.limit.output ? `${model.limit.output.toLocaleString()} tokens` : 'unknown')}
      {checkpointThreshold != null && context
        ? row('Checkpoint at', `≈ ${Math.round((context * checkpointThreshold) / 100).toLocaleString()} tokens`)
        : null}
      {row('Tool calling', capLabel(model.caps.tool_call))}
      {row('Image input', capLabel(model.caps.image_input))}
      {row('Reasoning', capLabel(model.caps.reasoning))}
      {row(
        'Price (per 1M in / out)',
        model.cost ? (
          <>
            {formatPrice(model.cost)}
            {model.cost.cache_read != null && <span style={{ color: 'var(--text-tertiary)' }}> · cached ${model.cost.cache_read}</span>}
          </>
        ) : (
          'unknown'
        )
      )}
      {model.knowledge && row('Knowledge', model.knowledge)}
      {model.release_date && row('Released', model.release_date)}
    </div>
  )
}

function isChatProvider(p: CatalogueProvider) {
  return p.dialect !== null || p.id === 'dummyplug'
}

/** A provider dropdown and a model dropdown, both from the catalogue (FR-005, FR-030). */
export default function ProviderModelPicker({
  value,
  onChange,
  filter,
  preselectDefault,
  showCard = true,
  invalid,
}: ProviderModelPickerProps) {
  const providers = useCatalogueProviders()
  const invalidate = useCatalogueStore((s) => s.invalidate)
  const [providerOpen, setProviderOpen] = useState(false)
  const [modelOpen, setModelOpen] = useState(false)
  const [providerQuery, setProviderQuery] = useState('')
  const [modelQuery, setModelQuery] = useState('')
  const [showAll, setShowAll] = useState(false)
  // The provider whose inline connect form was closed without connecting.
  const [dismissed, setDismissed] = useState<string | null>(null)
  const providerRef = useRef<HTMLDivElement>(null)
  const modelRef = useRef<HTMLDivElement>(null)
  useOutsideClick(providerRef, () => setProviderOpen(false))
  useOutsideClick(modelRef, () => setModelOpen(false))

  const models = useCatalogueModels(value.provider, showAll)
  const info = useModelInfo(value.provider, value.model)
  const selectedProvider = providers?.find((p) => p.id === value.provider)

  // FR-031: a provider with no model gets its default.
  useEffect(() => {
    if (preselectDefault && value.provider && !value.model && models?.length) {
      const preferred = models.find((m) => m.default) ?? models[0]
      if (preferred) onChange({ provider: value.provider, model: preferred.id })
    }
  }, [preselectDefault, value.provider, value.model, models])

  const chatProviders = useMemo(() => (providers || []).filter(isChatProvider), [providers])
  const q = providerQuery.trim().toLowerCase()
  const matching = chatProviders.filter(
    (p) => !q || p.name.toLowerCase().includes(q) || p.id.includes(q)
  )
  // A catalogue local provider (LM Studio, …) counts as connected without any setup; it is
  // listed as connected only once something was saved for it.
  const isConnected = (p: CatalogueProvider) =>
    p.connection.status === 'connected' && (p.connection.source !== null || p.origin === 'builtin')
  const groups: [string, CatalogueProvider[]][] = [
    ['CONNECTED', matching.filter((p) => p.origin !== 'custom' && isConnected(p))],
    ['CUSTOM', matching.filter((p) => p.origin === 'custom')],
    ['ALL PROVIDERS', matching.filter((p) => p.origin !== 'custom' && !isConnected(p))],
  ]

  const mq = modelQuery.trim().toLowerCase()
  const visibleModels = (models || []).filter((m) => {
    if (filter?.toolCallOnly && m.caps.tool_call === false) return false
    if (filter?.imageInputOnly && m.caps.image_input === false) return false
    return !mq || m.id.toLowerCase().includes(mq) || m.name.toLowerCase().includes(mq)
  })
  const typedIsNew = modelQuery.trim() && !(models || []).some((m) => m.id === modelQuery.trim())

  const pickProvider = (p: CatalogueProvider) => {
    setProviderOpen(false)
    setProviderQuery('')
    setDismissed(null)
    if (p.id !== value.provider) onChange({ provider: p.id, model: '' })
  }

  const pickModel = (id: string) => {
    setModelOpen(false)
    setModelQuery('')
    onChange({ provider: value.provider, model: id })
  }

  const connectingProvider =
    selectedProvider &&
    selectedProvider.connection.status === 'missing' &&
    selectedProvider.origin !== 'custom' &&
    dismissed !== selectedProvider.id
      ? selectedProvider
      : undefined
  const canConnect = hasPermission('settings:providers')

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.6rem' }}>
      <div style={{ display: 'flex', gap: '0.75rem', flexWrap: 'wrap' }}>
        {/* Provider */}
        <div ref={providerRef} style={{ position: 'relative', flex: '1 1 220px' }}>
          <button type="button" style={triggerStyle(invalid && !value.provider)} onClick={() => setProviderOpen((o) => !o)}>
            {value.provider ? (
              <>
                <ProviderLogo id={value.provider} name={selectedProvider?.name} src={selectedProvider?.logo} fullColor={selectedProvider?.logo_full_color} size={18} />
                <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                  {selectedProvider?.name ?? value.provider}
                </span>
                {selectedProvider?.connection.status === 'missing' && (
                  <span style={{ fontSize: '0.68rem', color: 'var(--warning, #d97706)' }}>not connected</span>
                )}
              </>
            ) : (
              <span style={{ flex: 1, color: 'var(--text-tertiary)' }}>Select provider</span>
            )}
            <FaChevronDown size={11} style={{ opacity: 0.5 }} />
          </button>
          {providerOpen && (
            <div style={popoverStyle} role="listbox">
              <SearchBox value={providerQuery} onChange={setProviderQuery} placeholder="Search providers" />
              <div style={{ maxHeight: LIST_HEIGHT, overflowY: 'auto' }}>
                {providers === null && <div style={{ padding: '0.75rem', fontSize: '0.8rem', color: 'var(--text-tertiary)' }}>Loading…</div>}
                {groups.map(([label, list]) =>
                  list.length ? (
                    <div key={label}>
                      <div style={groupStyle}>{label}</div>
                      {list.map((p) => (
                        <div
                          key={p.id}
                          role="option"
                          aria-selected={p.id === value.provider}
                          onClick={() => pickProvider(p)}
                          style={{
                            display: 'flex',
                            alignItems: 'center',
                            gap: '0.55rem',
                            padding: '0.45rem 0.75rem',
                            cursor: 'pointer',
                            fontSize: '0.85rem',
                            background: p.id === value.provider ? 'var(--hover)' : undefined,
                          }}
                          onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--hover)')}
                          onMouseLeave={(e) => (e.currentTarget.style.background = p.id === value.provider ? 'var(--hover)' : '')}
                        >
                          <ProviderLogo id={p.id} name={p.name} src={p.logo} fullColor={p.logo_full_color} size={18} />
                          <span style={{ flex: 1 }}>{p.name}</span>
                          {p.origin === 'custom' && <span style={{ fontSize: '0.65rem', color: 'var(--text-tertiary)', border: '1px solid var(--border)', borderRadius: 999, padding: '0 0.4rem' }}>custom</span>}
                          <span style={{ fontSize: '0.7rem', color: p.connection.status === 'connected' ? 'var(--success, #16a34a)' : 'var(--text-tertiary)' }}>
                            {p.connection.status === 'connected' ? (p.connection.source?.startsWith('env:') ? 'env' : '●') : '○'}
                          </span>
                        </div>
                      ))}
                    </div>
                  ) : null
                )}
                {providers && !matching.length && (
                  <div style={{ padding: '0.75rem', fontSize: '0.8rem', color: 'var(--text-tertiary)' }}>
                    No provider matches “{providerQuery}”. Add one under Settings › Providers › Custom provider.
                  </div>
                )}
              </div>
            </div>
          )}
        </div>

        {/* Model */}
        <div ref={modelRef} style={{ position: 'relative', flex: '2 1 280px' }}>
          <button
            type="button"
            style={triggerStyle(invalid && !value.model)}
            disabled={!value.provider}
            onClick={() => setModelOpen((o) => !o)}
          >
            {value.model ? (
              <>
                {info && <ProviderLogo id={info.id} name={info.name} src={info.logo} fullColor={info.logo_full_color} size={18} />}
                <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                  {info?.name ?? value.model}
                  {info && info.name !== value.model && (
                    <span style={{ marginLeft: 6, fontSize: '0.72rem', color: 'var(--text-tertiary)', fontFamily: 'var(--font-mono)' }}>{value.model}</span>
                  )}
                </span>
                {info === null && <span style={{ fontSize: '0.68rem', color: 'var(--text-tertiary)' }}>not in catalogue</span>}
                {info && <CapabilityIcons caps={info.caps} />}
              </>
            ) : (
              <span style={{ flex: 1, color: 'var(--text-tertiary)' }}>{value.provider ? 'Select model' : 'Choose a provider first'}</span>
            )}
            <FaChevronDown size={11} style={{ opacity: 0.5 }} />
          </button>
          {modelOpen && (
            <div style={popoverStyle} role="listbox">
              <SearchBox value={modelQuery} onChange={setModelQuery} placeholder="Search or type a model id" />
              <label style={{ display: 'flex', alignItems: 'center', gap: '0.4rem', padding: '0.4rem 0.75rem', fontSize: '0.75rem', color: 'var(--text-secondary)', borderBottom: '1px solid var(--border)', cursor: 'pointer' }}>
                <input type="checkbox" checked={showAll} onChange={(e) => setShowAll(e.target.checked)} />
                Show all models (including deprecated and ones without tool calling)
              </label>
              {typedIsNew && (
                <div
                  onClick={() => pickModel(modelQuery.trim())}
                  style={{ padding: '0.55rem 0.75rem', fontSize: '0.82rem', cursor: 'pointer', borderBottom: '1px solid var(--border)' }}
                  onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--hover)')}
                  onMouseLeave={(e) => (e.currentTarget.style.background = '')}
                >
                  Use “<code>{modelQuery.trim()}</code>” (not in the list)
                </div>
              )}
              {models === undefined ? (
                <div style={{ padding: '0.75rem', fontSize: '0.8rem', color: 'var(--text-tertiary)' }}>Loading…</div>
              ) : visibleModels.length ? (
                <VirtualList
                  items={visibleModels}
                  render={(m) => <ModelRow key={m.id} model={m} selected={m.id === value.model} onClick={() => pickModel(m.id)} />}
                />
              ) : (
                <div style={{ padding: '0.75rem', fontSize: '0.8rem', color: 'var(--text-tertiary)' }}>
                  {models.length ? 'No model matches.' : 'No models listed. Type a model id above.'}
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      {/* FR-034: connect a provider without leaving the form. */}
      {connectingProvider && (
        <div style={{ border: '1px dashed var(--border)', borderRadius: '0.5rem', padding: '0.75rem' }}>
          {canConnect ? (
            <ConnectForm
              provider={connectingProvider}
              compact
              onSaved={() => invalidate()}
              onCancel={() => setDismissed(connectingProvider.id)}
            />
          ) : (
            <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>
              {connectingProvider.name} isn't connected. Ask an admin to connect {connectingProvider.name}.
            </div>
          )}
        </div>
      )}

      {showCard && info && <ModelCard model={info} />}
    </div>
  )
}
