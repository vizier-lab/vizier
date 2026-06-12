import { useEffect, useState } from 'react'
import { FaDatabase } from 'react-icons/fa6'
import TooltipLabel from './TooltipLabel'
import { listLocalEmbeddingModels } from '../services/vizier'
import type {
  EmbeddingProvider,
  EmbeddingToolSettings,
  IndexerConfig,
  IndexerKind,
  LocalEmbeddingModel,
} from '../interfaces/types'
import { EMBEDDING_PROVIDERS } from '../interfaces/types'

interface EmbeddingIndexerSectionProps {
  embedding: EmbeddingToolSettings
  indexer: IndexerConfig
  onEmbeddingChange: (next: EmbeddingToolSettings) => void
  onIndexerChange: (next: IndexerConfig) => void
  inputStyle: React.CSSProperties
  labelStyle: React.CSSProperties
  fieldStyle: React.CSSProperties
}

export default function EmbeddingIndexerSection({
  embedding,
  indexer,
  onEmbeddingChange,
  onIndexerChange,
  inputStyle,
  labelStyle,
  fieldStyle,
}: EmbeddingIndexerSectionProps) {
  const [localModels, setLocalModels] = useState<LocalEmbeddingModel[]>([])

  useEffect(() => {
    let cancelled = false
    listLocalEmbeddingModels()
      .then((models) => {
        if (!cancelled) setLocalModels(models)
      })
      .catch(() => {
        // non-fatal: form still works with a text input fallback
      })
    return () => {
      cancelled = true
    }
  }, [])

  const updateProvider = (provider: EmbeddingProvider) => {
    const defaults: Record<EmbeddingProvider, EmbeddingToolSettings> = {
      local: { provider: 'local', model: 'all_mini_lml6_v2' },
      ollama: { provider: 'ollama', model: 'nomic-embed-text' },
      openai: { provider: 'openai', model: 'text-embedding-3-small' },
      gemini: { provider: 'gemini', model: 'text-embedding-004' },
      openrouter: { provider: 'openrouter', model: 'openai/text-embedding-3-small' },
    }
    onEmbeddingChange(defaults[provider])
  }

  const providerUsesLocalDropdown = embedding.provider === 'local'
  const providerIsOllama = embedding.provider === 'ollama'

  return (
    <>
      {/* Embedding & Indexer */}
      <div>
        <h4
          style={{
            fontSize: '0.85rem',
            fontWeight: 600,
            color: 'var(--text-primary)',
            marginBottom: '0.75rem',
            paddingBottom: '0.5rem',
            borderBottom: '1px solid var(--border)',
            display: 'flex',
            alignItems: 'center',
            gap: '0.5rem',
          }}
        >
          Embedding & Indexer
        </h4>
        <div
          style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '1rem',
          }}
        >
          <div
            style={{
              display: 'flex',
              gap: '0.75rem',
              alignItems: 'flex-end',
            }}
          >
            <section style={{ ...fieldStyle, flex: 1 }}>
              <label style={labelStyle}>
                <TooltipLabel
                  label="Embedding Provider"
                  tooltip="Provider used to embed memory content. API keys are managed in the Providers settings page."
                />
              </label>
              <select
                style={inputStyle}
                value={embedding.provider}
                onChange={(e) =>
                  updateProvider(e.target.value as EmbeddingProvider)
                }
              >
                {EMBEDDING_PROVIDERS.map((p) => (
                  <option key={p} value={p}>
                    {p}
                  </option>
                ))}
              </select>
            </section>
            <section style={{ ...fieldStyle, flex: 2 }}>
              <label style={labelStyle}>
                <TooltipLabel
                  label="Embedding Model"
                  tooltip="The embedding model identifier. For 'local' provider, pick from the dropdown (29 models). For others, enter the model name (e.g. 'text-embedding-3-small' for OpenAI)."
                />
              </label>
              {providerUsesLocalDropdown ? (
                localModels.length > 0 ? (
                  <select
                    style={inputStyle}
                    value={embedding.model}
                    onChange={(e) =>
                      onEmbeddingChange({
                        ...embedding,
                        model: e.target.value,
                      })
                    }
                  >
                    {localModels.map((m) => (
                      <option key={m.variant} value={m.variant}>
                        {m.name} ({m.tier})
                      </option>
                    ))}
                  </select>
                ) : (
                  <input
                    style={inputStyle}
                    value={embedding.model}
                    onChange={(e) =>
                      onEmbeddingChange({
                        ...embedding,
                        model: e.target.value,
                      })
                    }
                  />
                )
              ) : (
                <input
                  style={inputStyle}
                  value={embedding.model}
                  onChange={(e) =>
                    onEmbeddingChange({
                      ...embedding,
                      model: e.target.value,
                    })
                  }
                />
              )}
            </section>
          </div>
          {providerIsOllama && (
            <section style={fieldStyle}>
              <label style={labelStyle}>
                <TooltipLabel
                  label="Ollama Base URL"
                  tooltip="Optional per-agent override. Falls back to the Ollama base URL configured in the Providers settings page, or http://localhost:11434."
                />
              </label>
              <input
                style={inputStyle}
                placeholder="http://localhost:11434"
                value={embedding.base_url || ''}
                onChange={(e) =>
                  onEmbeddingChange({
                    ...embedding,
                    base_url: e.target.value || undefined,
                  })
                }
              />
            </section>
          )}
          <section style={fieldStyle}>
            <label style={labelStyle}>
              <TooltipLabel
                label="Indexer"
                tooltip="Where vector embeddings are stored. Surreal is the only indexer available today; this field is reserved for future kinds (e.g. in-memory, remote)."
              />
            </label>
            <select
              style={inputStyle}
              value={indexer.kind}
              onChange={(e) =>
                onIndexerChange({ kind: e.target.value as IndexerKind })
              }
            >
              <option value="surreal">Surreal (vector store)</option>
            </select>
          </section>
        </div>
      </div>
    </>
  )
}
