import type { AgentDetail, AgentInput } from '../api/endpoints/agents'
import type { ModelSummary } from '../api/endpoints/catalogue'

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
}

export const TTS_PROVIDERS = ['openai', 'openrouter', 'elevenlabs', 'xai', 'hyperbolic', 'kokoro']
export const STT_PROVIDERS = ['openai', 'openrouter', 'elevenlabs', 'groq', 'mistral', 'huggingface', 'gemini', 'whisper']
export const IMAGE_GEN_PROVIDERS = ['openai', 'openrouter', 'xai', 'huggingface', 'hyperbolic', 'bedrock']
/** What each provider uses when the model is left empty (mirrors the server's defaults). */
export const STT_DEFAULT_MODELS: Record<string, string> = {
    openai: 'whisper-1',
    openrouter: 'openai/whisper-large-v3',
    elevenlabs: 'scribe_v1',
    groq: 'whisper-large-v3',
    mistral: 'voxtral-mini-2507',
    huggingface: 'openai/whisper-large-v3',
    gemini: 'gemini-1.5-flash',
    whisper: 'ggml-small',
}
export const IMAGE_GEN_DEFAULT_MODELS: Record<string, string> = {
    openai: 'dall-e-3',
    openrouter: 'bytedance-seed/seedream-4.5',
    xai: 'grok-2-image-1212',
    huggingface: 'stabilityai/stable-diffusion-xl-base-1.0',
    hyperbolic: 'SDXL1.0-base',
    bedrock: 'amazon.nova-canvas-v1:0',
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
/** GGML Whisper models whisper.cpp publishes, smallest first (sizes approximate). */
export const WHISPER_MODELS = [
    { value: 'ggml-tiny', label: 'ggml-tiny · 75 MB' },
    { value: 'ggml-tiny.en', label: 'ggml-tiny.en · 75 MB, English only' },
    { value: 'ggml-base', label: 'ggml-base · 142 MB' },
    { value: 'ggml-base.en', label: 'ggml-base.en · 142 MB, English only' },
    { value: 'ggml-small', label: 'ggml-small · 466 MB' },
    { value: 'ggml-small.en', label: 'ggml-small.en · 466 MB, English only' },
    { value: 'ggml-medium', label: 'ggml-medium · 1.5 GB' },
    { value: 'ggml-medium.en', label: 'ggml-medium.en · 1.5 GB, English only' },
    { value: 'ggml-large-v3-turbo', label: 'ggml-large-v3-turbo · 1.6 GB' },
    { value: 'ggml-large-v3', label: 'ggml-large-v3 · 3.1 GB' },
]
export const KOKORO_MODELS = [{ value: 'Kokoro-82M', label: 'Kokoro-82M · 327 MB' }]
/** The voice packs Kokoro-82M ships; mirrors `VOICES` in src/tts/kokoro.rs. */
export const KOKORO_VOICES = [
    'af_alloy', 'af_aoede', 'af_bella', 'af_heart', 'af_jessica', 'af_kore', 'af_nicole', 'af_nova', 'af_river', 'af_sarah', 'af_sky',
    'am_adam', 'am_echo', 'am_eric', 'am_fenrir', 'am_liam', 'am_michael', 'am_onyx', 'am_puck', 'am_santa',
    'bf_alice', 'bf_emma', 'bf_isabella', 'bf_lily', 'bm_daniel', 'bm_fable', 'bm_george', 'bm_lewis',
    'ef_dora', 'em_alex', 'em_santa', 'ff_siwis', 'hf_alpha', 'hf_beta', 'hm_omega', 'hm_psi', 'if_sara', 'im_nicola',
    'jf_alpha', 'jf_gongitsune', 'jf_nezumi', 'jf_tebukuro', 'jm_kumo', 'pf_dora', 'pm_alex', 'pm_santa',
    'zf_xiaobei', 'zf_xiaoni', 'zf_xiaoxiao', 'zf_xiaoyi', 'zm_yunjian', 'zm_yunxi', 'zm_yunxia', 'zm_yunyang',
].map((v) => ({ value: v, label: v }))
export const EMBEDDING_PROVIDERS = ['local', 'openrouter', 'ollama', 'openai', 'gemini', 'voyageai', 'mistral', 'together', 'cohere', 'copilot', 'bedrock']
/** Providers with a documented default when the model is left empty (mirrors the server). */
export const EMBEDDING_DEFAULT_MODELS: Record<string, string> = {
    bedrock: 'amazon.titan-embed-text-v2:0',
}
/** What a local index uses when no model is named (the server's own default). */
export const LOCAL_EMBEDDING_DEFAULT = 'all_mini_lml6_v2'
/**
 * The fastembed models a local index can use, with the server's own tier for each: mirrors
 * `LOCAL_EMBEDDING_MODELS` in `crates/vizier-core/src/config/embedding.rs`, in its order.
 */
export const LOCAL_EMBEDDING_MODELS = ([
    ['all_mini_lml6_v2', 'lightweight'],
    ['all_mini_lml6_v2q', 'lightweight'],
    ['all_mini_lml12_v2', 'performance'],
    ['all_mini_lml12_v2q', 'balanced'],
    ['bge_base_env15', 'balanced'],
    ['bge_base_env15q', 'balanced'],
    ['bge_large_env15', 'performance'],
    ['bge_large_env15q', 'balanced'],
    ['bge_small_env15', 'lightweight'],
    ['bge_small_env15q', 'lightweight'],
    ['nomic_embed_text_v1', 'balanced'],
    ['nomic_embed_text_v15', 'balanced'],
    ['nomic_embed_text_v15q', 'balanced'],
    ['paraphrase_ml_mini_lml12_v2', 'lightweight'],
    ['paraphrase_ml_mini_lml12_v2q', 'lightweight'],
    ['paraphrase_ml_mpnet_base_v2', 'balanced'],
    ['bge_small_zh_v15', 'lightweight'],
    ['bge_large_zh_v15', 'performance'],
    ['modernbert_embed_large', 'performance'],
    ['multilingual_e5_small', 'lightweight'],
    ['multilingual_e5_base', 'balanced'],
    ['multilingual_e5_large', 'performance'],
    ['mxbai_embed_large_v1', 'performance'],
    ['mxbai_embed_large_v1q', 'balanced'],
    ['gte_base_env15', 'balanced'],
    ['gte_base_env15q', 'balanced'],
    ['gte_large_env15', 'performance'],
    ['gte_large_env15q', 'balanced'],
    ['clip_vit_b32', 'balanced'],
    ['jina_embeddings_v2_base_code', 'balanced'],
] as const).map(([value, tier]) => ({ value, label: `${value} · ${tier}` }))

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

/**
 * The catalogue's own limits for a model, as the Context window and Max output fields hold them.
 * A model the catalogue doesn't know (a typed id) has none, so both come back empty.
 */
export function modelLimits(info?: ModelSummary): { contextWindow: string; maxTokens: string } {
    const context = info?.limit.input ?? info?.limit.context
    return {
        contextWindow: context ? String(context) : '',
        maxTokens: info?.limit.output ? String(info.limit.output) : '',
    }
}

/**
 * What Context window and Max output become when a model is chosen: that model's own numbers, except
 * where a field holds a value someone typed, which stays as it is. `auto` is what the previous choice
 * wrote, so "still holding what we filled in" is told apart from "chosen on purpose".
 */
export function refilledLimits(
    current: Pick<AgentForm, 'contextWindow' | 'maxTokens'>,
    auto: { contextWindow: string; maxTokens: string },
    info?: ModelSummary
): Pick<AgentForm, 'contextWindow' | 'maxTokens'> {
    const limits = modelLimits(info)
    const patch = { contextWindow: current.contextWindow, maxTokens: current.maxTokens }
    for (const key of ['contextWindow', 'maxTokens'] as const) {
        if (!current[key] || current[key] === auto[key]) patch[key] = limits[key]
    }
    return patch
}

const num = (s: string): number | null => {
    const n = Number(s)
    return s.trim() !== '' && Number.isFinite(n) ? n : null
}

/** The first thing wrong with the fields a step needs, or nothing. */
export function problems(f: AgentForm, step: 'config' | 'models' | 'prompt' | 'addons' | 'all', editing = false): Record<string, string> {
    const out: Record<string, string> = {}
    if (step === 'config' || step === 'all') {
        if (!f.name.trim()) out.name = 'Give the agent a name.'
        if (!editing) {
            if (!f.agentId) out.agentId = 'An id is needed.'
            else if (!AGENT_ID.test(f.agentId)) out.agentId = 'Lowercase letters, numbers, - and _.'
        }
    }
    if (step === 'models' || step === 'all') {
        if (!f.provider) out.provider = 'Choose a provider.'
        if (!f.model.trim()) out.model = 'Choose a model.'
        for (const key of ['contextWindow', 'maxTokens'] as const) {
            if (f[key].trim() !== '' && (num(f[key]) ?? -1) <= 0) out[key] = 'A positive number, or empty.'
        }
        if (f.tts && f.ttsSpeed.trim() !== '' && (num(f.ttsSpeed) ?? -1) <= 0) out.ttsSpeed = 'A positive number, or empty.'
        if (f.dreamEnabled && !f.dreamSchedule.trim()) out.dreamSchedule = 'A cron expression is needed when dreams are on.'
        if (f.readImage && f.visionOwn && (!f.visionProvider || !f.visionModel)) out.vision = 'Choose a vision model.'
        // The memory index's model sits in Models too, and only Bedrock and the built-in local index
        // have a documented default.
        if (f.embeddingProvider !== 'local' && f.embeddingProvider !== 'bedrock' && !f.embeddingModel.trim()) out.embeddingModel = 'Name the embedding model.'
    }
    return out
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
        // The dream model is kept even while dreams are off, so choosing one in Models sticks. One
        // that is the language model is sent as none, so it keeps following it when that changes.
        dream_provider: ownDreamModel(f) ? f.dreamProvider || null : null,
        dream_model: ownDreamModel(f) ? f.dreamModel || null : null,
        avatar_url: f.avatarUrl || null,
        silent_read_initiative_chance: (num(f.silentReadPercent) ?? 0) / 100,
        // The server wants a model with a provider; none means its own default (and Bedrock's
        // empty model is meaningful: it selects Titan Text Embeddings V2).
        embedding:
            f.embeddingModel.trim() || f.embeddingProvider === 'bedrock'
                ? { provider: f.embeddingProvider, model: f.embeddingModel.trim() }
                : null,
        indexer: base?.indexer ?? undefined,
        tools: {
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

/**
 * True when the dream cycle has a model of its own. The form defaults it to the language model, and
 * that pair is sent as "none" so it keeps following the language model instead of freezing on it.
 */
function ownDreamModel(f: AgentForm): boolean {
    return f.dreamProvider !== f.provider || f.dreamModel !== f.model
}

const str = (v: unknown, fallback = '') => (typeof v === 'string' ? v : fallback)

export function fromDetail(d: AgentDetail): AgentForm {
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
    }
}
