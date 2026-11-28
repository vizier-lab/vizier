// The providers a media capability (speech, transcription, pictures, embeddings) can use, matched
// to their catalogue entries so the picker can show logos and connection state.
import type { ProviderSummary } from '../api/endpoints/catalogue'

/**
 * The catalogue id behind each setting value, as `credentials()` on the server's provider enums
 * maps it (`crates/vizier-core/src/schema/agent.rs`). A value missing here runs on this machine
 * and needs no connection: kokoro, whisper and the built-in embeddings.
 */
export const CATALOGUE_ID: Record<string, string> = {
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
    venice: 'venice',
    bedrock: 'amazon-bedrock',
    voyageai: 'voyageai',
    cohere: 'cohere',
    copilot: 'github-copilot',
    ollama: 'ollama',
}

/** What a setting value that runs on this machine is called in the list. */
const LOCAL_NAMES: Record<string, string> = {
    kokoro: 'Kokoro',
    whisper: 'Whisper',
    local: 'Built-in',
}

export type MediaProviderRow = {
    /** The setting value saved on the agent. */
    value: string
    name: string
    /** The catalogue entry, when the provider has one (for its logo and connection). */
    entry?: ProviderSummary
    local: boolean
    connected: boolean
}

/** A catalogue local provider counts as connected without any setup. */
export const isConnected = (p: ProviderSummary) =>
    p.connection.status === 'connected' && (p.connection.source != null || p.origin === 'builtin')

/** One row per allowed setting value, in the list's own order. */
export function mediaProviderRows(values: string[], catalogue: ProviderSummary[] | undefined): MediaProviderRow[] {
    return values.map((value) => {
        const id = CATALOGUE_ID[value]
        if (!id) return { value, name: LOCAL_NAMES[value] ?? value, local: true, connected: true }
        const entry = catalogue?.find((p) => p.id === id)
        return { value, name: entry?.name ?? value, entry, local: false, connected: entry ? isConnected(entry) : false }
    })
}

/** Grouped as the language model's picker is: on this machine, connected, then the rest. */
export function mediaProviderSections(rows: MediaProviderRow[], query: string) {
    const q = query.trim().toLowerCase()
    const match = rows.filter((r) => !q || r.name.toLowerCase().includes(q) || r.value.includes(q))
    return [
        { label: 'On this machine', items: match.filter((r) => r.local) },
        { label: 'Connected', items: match.filter((r) => !r.local && r.connected) },
        { label: 'All providers', items: match.filter((r) => !r.local && !r.connected) },
    ]
}

/** The catalogue entry still to connect before the chosen provider can work, if any. */
export const needsConnecting = (row: MediaProviderRow | undefined) =>
    row && !row.local && row.entry && row.entry.connection.status === 'missing' && row.entry.origin !== 'custom' ? row.entry : undefined
