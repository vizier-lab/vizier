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
    /** Empty means the provider's default. */
    ttsModel: string
    ttsVoice: string
    /** A multiplier such as 1.2; empty means the provider's own. */
    ttsSpeed: string
    stt: boolean
    sttProvider: string
    sttModel: string
    /** Language hint such as `en`; empty lets the model detect it. */
    sttLanguage: string
    readImage: boolean
    /** True when the vision model is its own pick; false sends images to the main model. */
    visionOwn: boolean
    visionProvider: string
    visionModel: string
    imageGen: boolean
    imageGenProvider: string
    imageGenModel: string
    /** Such as `1024x1024`; empty means the provider's default. */
    imageGenSize: string
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
export const STT_PROVIDERS = ['openai', 'openrouter', 'elevenlabs', 'groq', 'mistral', 'huggingface', 'gemini', 'whisper']
export const IMAGE_GEN_PROVIDERS = ['openai', 'openrouter', 'xai', 'huggingface', 'hyperbolic']
/** What each provider uses when the model is left empty (mirrors the server's defaults). */
export const STT_DEFAULT_MODELS: Record<string, string> = {
    openai: 'whisper-1',
    openrouter: 'openai/whisper-large-v3',
    elevenlabs: 'scribe_v1',
    groq: 'whisper-large-v3',
    mistral: 'voxtral-mini-2507',
    huggingface: 'openai/whisper-large-v3',
    gemini: 'gemini-1.5-flash',
    whisper: 'ggml-large-v3',
}
export const IMAGE_GEN_DEFAULT_MODELS: Record<string, string> = {
    openai: 'dall-e-3',
    openrouter: 'bytedance-seed/seedream-4.5',
    xai: 'grok-2-image-1212',
    huggingface: 'stabilityai/stable-diffusion-xl-base-1.0',
    hyperbolic: 'SDXL1.0-base',
}
export const TTS_DEFAULT_MODELS: Record<string, string> = {
    openai: 'tts-1',
    openrouter: 'hexgrad/kokoro-82m',
    elevenlabs: 'eleven_multilingual_v2',
    xai: 'grok-2-tts',
    hyperbolic: 'Melo-TTS',
    kokoro: 'Kokoro-82M',
}
export const TTS_DEFAULT_VOICES: Record<string, string> = {
    openai: 'alloy',
    openrouter: 'af_alloy',
    elevenlabs: 'pqHfZKP75CvOlQylNhV4',
    xai: 'default',
    hyperbolic: 'default',
    kokoro: 'af',
}
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
    ttsProvider: 'kokoro',
    ttsModel: '',
    ttsVoice: '',
    ttsSpeed: '',
    stt: false,
    sttProvider: 'whisper',
    sttModel: '',
    sttLanguage: '',
    readImage: false,
    visionOwn: false,
    visionProvider: '',
    visionModel: '',
    imageGen: false,
    imageGenProvider: 'openai',
    imageGenModel: '',
    imageGenSize: '',
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
        if (f.tts && f.ttsSpeed.trim() !== '' && (num(f.ttsSpeed) ?? -1) <= 0) out.ttsSpeed = 'A positive number, or empty.'
        if (f.readImage && f.visionOwn && (!f.visionProvider || !f.visionModel)) out.vision = 'Choose a vision model.'
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
            tts_settings: f.tts
                ? {
                      ...keep(base?.tts_settings, f.ttsProvider),
                      model: f.ttsModel.trim() || null,
                      voice: f.ttsVoice.trim() || null,
                      speed: num(f.ttsSpeed),
                  }
                : (base?.tts_settings ?? null),
            stt: f.stt,
            stt_settings: f.stt
                ? { ...keep(base?.stt_settings, f.sttProvider), model: f.sttModel.trim() || null, language: f.sttLanguage.trim() || null }
                : (base?.stt_settings ?? null),
            read_image: f.readImage,
            // No vision model of its own: images go to the main model as attachments.
            read_image_settings: f.readImage
                ? f.visionOwn
                    ? { provider: f.visionProvider, model: f.visionModel }
                    : { provider: null, model: null }
                : (base?.read_image_settings ?? null),
            image_gen: f.imageGen,
            image_gen_settings: f.imageGen
                ? { ...keep(base?.image_gen_settings, f.imageGenProvider), model: f.imageGenModel.trim() || null, size: f.imageGenSize.trim() || null }
                : (base?.image_gen_settings ?? null),
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
        ttsProvider: str(d.tts_settings?.provider, 'kokoro'),
        ttsModel: str(d.tts_settings?.model),
        ttsVoice: str(d.tts_settings?.voice),
        ttsSpeed: typeof d.tts_settings?.speed === 'number' ? String(d.tts_settings.speed) : '',
        stt: d.stt,
        sttProvider: str(d.stt_settings?.provider, 'whisper'),
        sttModel: str(d.stt_settings?.model),
        sttLanguage: str(d.stt_settings?.language),
        readImage: d.read_image,
        visionOwn: !!(d.read_image_settings?.provider && d.read_image_settings?.model),
        visionProvider: d.read_image_settings?.provider ?? '',
        visionModel: d.read_image_settings?.model ?? '',
        imageGen: d.image_gen,
        imageGenProvider: str(d.image_gen_settings?.provider, 'openai'),
        imageGenModel: str(d.image_gen_settings?.model),
        imageGenSize: str(d.image_gen_settings?.size),
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
