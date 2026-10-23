import { useState } from 'react'
import SlideOver from '../SlideOver'
import { ModelRow, VirtualList } from '../ProviderModelPicker'
import { useCatalogueModels } from '../../hooks/catalogueStore'
import { inputStyle } from './styles'

/** Every model a provider has, deprecated ones marked, read-only. */
export default function ModelsDrawer({ provider, name, onClose }: { provider: string | null; name?: string; onClose: () => void }) {
  const models = useCatalogueModels(provider || '', true)
  const [q, setQ] = useState('')
  const query = q.trim().toLowerCase()
  const shown = (models || []).filter((m) => !query || m.id.toLowerCase().includes(query) || m.name.toLowerCase().includes(query))
  return (
    <SlideOver open={!!provider} onClose={onClose} title={`${name ?? provider} models`}>
      <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
        <input style={inputStyle} placeholder="Search models" value={q} onChange={(e) => setQ(e.target.value)} />
        <div style={{ fontSize: '0.75rem', color: 'var(--text-tertiary)' }}>
          {models === undefined ? 'Loading…' : `${shown.length} of ${models.length} models`}
        </div>
        <div style={{ border: '1px solid var(--border)', borderRadius: '0.5rem', overflow: 'hidden' }}>
          <VirtualList items={shown} height={560} render={(m) => <ModelRow key={m.id} model={m} />} />
        </div>
      </div>
    </SlideOver>
  )
}
