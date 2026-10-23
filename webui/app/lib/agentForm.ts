import type { AgentDetail, AgentInput, ShellConfig } from '../api/endpoints/agents'

/** What the New agent wizard and Config edit: one flat object, converted to the request on save. */
export type AgentForm = {
    agentId: string
    name: string
    description: string
    provider: string
    model: string
    systemPrompt: string
    avatarUrl: string
    contextWindow: string
    maxTokens: string
    thinkingDepth: string
    promptTimeout: string
    toolsTimeout: string
    /** 0 to 100 (%). */
    checkpointPercent: string
    dreamEnabled: boolean
    dreamSchedule: string
    dreamProvider: string
    dreamModel: string
    /** 0 to 100 (%). */
    silentReadPercent: string
    embeddingProvider: string
    embeddingModel: string
    fetch: boolean
    httpClient: boolean
    tts: boolean
    ttsProvider: string
    stt: boolean
    sttProvider: string
    readImage: boolean
    visionProvider: string
    visionModel: string
    imageGen: boolean
    imageGenProvider: string
    shell: boolean
    shellKind: 'local' | 'docker'
    shellPath: string
    dockerSource: 'pull' | 'dockerfile'
    dockerImage: string
    dockerfilePath: string
    containerName: string
    shellEnv: string
}

export const TTS_PROVIDERS = ['openai', 'openrouter', 'elevenlabs', 'xai', 'hyperbolic', 'kokoro']
export const STT_PROVIDERS = ['openai', 'elevenlabs', 'groq', 'mistral', 'huggingface', 'gemini', 'whisper']
export const IMAGE_GEN_PROVIDERS = ['openai', 'xai', 'huggingface', 'hyperbolic']
export const EMBEDDING_PROVIDERS = ['local', 'openrouter', 'ollama', 'openai', 'gemini', 'voyageai', 'mistral', 'together', 'cohere', 'copilot']

export const emptyForm = (): AgentForm => ({
    agentId: '',
    name: '',
    description: '',
    provider: '',
    model: '',
    systemPrompt: '',
    avatarUrl: '',
    contextWindow: '',
    maxTokens: '',
    thinkingDepth: '100',
    promptTimeout: '10m',
    toolsTimeout: '30m',
    checkpointPercent: '80',
    dreamEnabled: false,
    dreamSchedule: '0 3 * * *',
    dreamProvider: '',
    dreamModel: '',
    silentReadPercent: '0',
    embeddingProvider: 'local',
    embeddingModel: '',
    fetch: false,
    httpClient: false,
    tts: false,
    ttsProvider: 'openai',
    stt: false,
    sttProvider: 'openai',
    readImage: false,
    visionProvider: '',
    visionModel: '',
    imageGen: false,
    imageGenProvider: 'openai',
    shell: false,
    shellKind: 'local',
    shellPath: '',
    dockerSource: 'pull',
    dockerImage: 'ubuntu:latest',
    dockerfilePath: '',
    containerName: 'vizier',
    shellEnv: '',
})

/** "Meeting Notes" → "meeting-notes": the agent id a name suggests. */
export function slugOf(name: string): string {
    return name
        .toLowerCase()
        .normalize('NFKD')
        .replace(/[̀-ͯ]/g, '')
        .replace(/[^a-z0-9]+/g, '-')
        .replace(/^-+|-+$/g, '')
        .slice(0, 40)
}

export const AGENT_ID = /^[a-z0-9][a-z0-9_-]*$/

/** `KEY=value` lines to a map; blank lines and `#` comments are skipped. */
export function parseEnv(text: string): Record<string, string> | null {
    const env: Record<string, string> = {}
    for (const raw of text.split('\n')) {
        const line = raw.trim()
        if (!line || line.startsWith('#')) continue
        const at = line.indexOf('=')
        if (at < 1) continue
        env[line.slice(0, at).trim()] = line.slice(at + 1).trim()
    }
    return Object.keys(env).length ? env : null
}

export const formatEnv = (env?: Record<string, string> | null) =>
    Object.entries(env ?? {})
        .map(([k, v]) => `${k}=${v}`)
        .join('\n')

const num = (s: string): number | null => {
    const n = Number(s)
    return s.trim() !== '' && Number.isFinite(n) ? n : null
}

/** The first thing wrong with the fields a step needs, or nothing. */
export function problems(f: AgentForm, step: 'config' | 'prompt' | 'tools' | 'all', editing = false): Record<string, string> {
    const out: Record<string, string> = {}
    if (step === 'config' || step === 'all') {
        if (!f.name.trim()) out.name = 'Give the agent a name.'
        if (!editing) {
            if (!f.agentId) out.agentId = 'An id is needed.'
            else if (!AGENT_ID.test(f.agentId)) out.agentId = 'Lowercase letters, numbers, - and _.'
        }
        if (!f.provider) out.provider = 'Choose a provider.'
        if (!f.model.trim()) out.model = 'Choose a model.'
        for (const key of ['contextWindow', 'maxTokens'] as const) {
            if (f[key].trim() !== '' && (num(f[key]) ?? -1) <= 0) out[key] = 'A positive number, or empty.'
        }
    }
    if (step === 'tools' || step === 'all') {
        if (f.shell && f.shellKind === 'local' && !f.shellPath.trim()) out.shellPath = 'Where the shell starts.'
        if (f.shell && f.shellKind === 'docker' && f.dockerSource === 'pull' && !f.dockerImage.trim()) out.dockerImage = 'An image to pull.'
        if (f.shell && f.shellKind === 'docker' && f.dockerSource === 'dockerfile' && !f.dockerfilePath.trim()) out.dockerfilePath = 'Path to the Dockerfile.'
        if (f.readImage && (!f.visionProvider || !f.visionModel)) out.vision = 'Choose a vision model.'
    }
    if (step === 'all' && f.embeddingProvider !== 'local' && !f.embeddingModel.trim()) out.embeddingModel = 'Name the embedding model.'
    return out
}

export function shellOf(f: AgentForm): ShellConfig | null {
    if (!f.shell) return null
    const env = parseEnv(f.shellEnv)
    if (f.shellKind === 'local') return { environment: 'local', path: f.shellPath.trim(), env }
    return {
        environment: 'docker',
        image:
            f.dockerSource === 'pull'
                ? { source: 'pull', name: f.dockerImage.trim() }
                : { source: 'dockerfile', path: f.dockerfilePath.trim(), name: f.dockerImage.trim() || f.containerName.trim() },
        container_name: f.containerName.trim() || 'vizier',
        env,
    }
}

/**
 * The request body. Settings the form has no field for (a voice, a size, a model) are carried over
 * from `base`, the agent as last loaded, so saving never clears what the form never showed.
 */
export function toInput(f: AgentForm, base?: AgentDetail): AgentInput {
    const keep = (current: Record<string, unknown> | null | undefined, provider: string) => ({
        ...(current ?? {}),
        provider,
        // A different provider has its own models and voices.
        ...(current?.provider !== provider ? { model: null, voice: null } : {}),
    })
    return {
        agent_id: f.agentId,
        name: f.name.trim(),
        description: f.description.trim() || null,
        provider: f.provider,
        model: f.model.trim(),
        system_prompt: f.systemPrompt.trim() ? f.systemPrompt : null,
        thinking_depth: num(f.thinkingDepth) ?? 100,
        checkpoint_threshold: (num(f.checkpointPercent) ?? 80) / 100,
        max_tokens: num(f.maxTokens),
        context_window: num(f.contextWindow),
        prompt_timeout: f.promptTimeout.trim() || '10m',
        dream_enabled: f.dreamEnabled,
        dream_schedule: f.dreamEnabled ? f.dreamSchedule.trim() || null : (base?.dream_schedule ?? null),
        dream_provider: f.dreamEnabled && f.dreamProvider ? f.dreamProvider : null,
        dream_model: f.dreamEnabled && f.dreamModel ? f.dreamModel : null,
        avatar_url: f.avatarUrl || null,
        silent_read_initiative_chance: (num(f.silentReadPercent) ?? 0) / 100,
        // The server wants a model with a provider; none means its own default.
        embedding: f.embeddingModel.trim() ? { provider: f.embeddingProvider, model: f.embeddingModel.trim() } : null,
        indexer: base?.indexer ?? undefined,
        tools: {
            shell: shellOf(f),
            fetch: f.fetch,
            http_client: f.httpClient,
            timeout: f.toolsTimeout.trim() || '30m',
            tts: f.tts,
            tts_settings: f.tts ? keep(base?.tts_settings, f.ttsProvider) : (base?.tts_settings ?? null),
            stt: f.stt,
            stt_settings: f.stt ? keep(base?.stt_settings, f.sttProvider) : (base?.stt_settings ?? null),
            read_image: f.readImage,
            read_image_settings: f.readImage ? { provider: f.visionProvider, model: f.visionModel } : (base?.read_image_settings ?? null),
            image_gen: f.imageGen,
            image_gen_settings: f.imageGen ? keep(base?.image_gen_settings, f.imageGenProvider) : (base?.image_gen_settings ?? null),
        },
    }
}

const str = (v: unknown, fallback = '') => (typeof v === 'string' ? v : fallback)

export function fromDetail(d: AgentDetail): AgentForm {
    const shell = d.shell
    return {
        ...emptyForm(),
        agentId: d.agent_id,
        name: d.name,
        description: d.description ?? '',
        provider: d.provider,
        model: d.model,
        systemPrompt: d.system_prompt ?? '',
        avatarUrl: d.avatar_url ?? '',
        contextWindow: d.context_window ? String(d.context_window) : '',
        maxTokens: d.max_tokens ? String(d.max_tokens) : '',
        thinkingDepth: String(d.thinking_depth),
        promptTimeout: d.prompt_timeout,
        toolsTimeout: d.tools_timeout,
        checkpointPercent: String(Math.round(d.checkpoint_threshold * 100)),
        dreamEnabled: d.dream_enabled,
        dreamSchedule: d.dream_schedule ?? emptyForm().dreamSchedule,
        dreamProvider: d.dream_provider ?? '',
        dreamModel: d.dream_model ?? '',
        silentReadPercent: String(Math.round(d.silent_read_initiative_chance * 100)),
        embeddingProvider: d.embedding?.provider ?? 'local',
        embeddingModel: d.embedding?.model ?? '',
        fetch: d.fetch,
        httpClient: d.http_client,
        tts: d.tts,
        ttsProvider: str(d.tts_settings?.provider, 'openai'),
        stt: d.stt,
        sttProvider: str(d.stt_settings?.provider, 'openai'),
        readImage: d.read_image,
        visionProvider: d.read_image_settings?.provider ?? '',
        visionModel: d.read_image_settings?.model ?? '',
        imageGen: d.image_gen,
        imageGenProvider: str(d.image_gen_settings?.provider, 'openai'),
        shell: !!shell,
        shellKind: shell?.environment ?? 'local',
        shellPath: shell?.environment === 'local' ? shell.path : '',
        dockerSource: shell?.environment === 'docker' ? shell.image.source : 'pull',
        dockerImage: shell?.environment === 'docker' ? (shell.image.source === 'pull' ? shell.image.name : shell.image.name) : 'ubuntu:latest',
        dockerfilePath: shell?.environment === 'docker' && shell.image.source === 'dockerfile' ? shell.image.path : '',
        containerName: shell?.environment === 'docker' ? shell.container_name : 'vizier',
        shellEnv: formatEnv(shell?.env),
    }
}
