import { Link } from 'react-router'
import { useCatalogueProviders } from '../hooks/catalogueStore'

/** Whose credentials each media provider uses: the client mirror of `credentials()` in
 *  `src/schema/agent.rs` (data-model §8). `null`: runs locally, no key. */
const CREDENTIALS: Record<string, string | null> = {
  openai: 'openai',
  openrouter: 'openrouter',
  elevenlabs: 'elevenlabs',
  xai: 'xai',
  hyperbolic: 'hyperbolic',
  groq: 'groq',
  mistral: 'mistral',
  huggingface: 'huggingface',
  gemini: 'google',
  together: 'togetherai',
  copilot: 'github-copilot',
  cohere: 'cohere',
  voyageai: 'voyageai',
  ollama: 'ollama',
  kokoro: null,
  whisper: null,
  local: null,
}

export function mediaCredentials(provider: string): string | null {
  return provider in CREDENTIALS ? CREDENTIALS[provider] : provider
}

/** "✓ uses your OpenAI key", or a link to connect the provider (FR-033 follow-on). */
export default function MediaKeyHint({ provider }: { provider: string }) {
  const providers = useCatalogueProviders()
  const id = mediaCredentials(provider)
  if (!id || !providers) return null
  const p = providers.find((x) => x.id === id)
  const name = p?.name ?? id
  if (p?.connection.status === 'connected') {
    return (
      <div style={{ fontSize: '0.75rem', color: 'var(--success, #16a34a)' }}>
        ✓ uses your {name} {p.connection.source?.startsWith('env:') ? `key (${p.connection.source.slice(4)})` : 'key'}
      </div>
    )
  }
  return (
    <div style={{ fontSize: '0.75rem', color: 'var(--warning, #d97706)' }}>
      {name} isn't connected.{' '}
      <Link to="/settings?tab=providers" style={{ color: 'var(--accent-primary)' }}>
        Connect {name}
      </Link>
    </div>
  )
}
