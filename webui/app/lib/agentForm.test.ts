import { describe, expect, it } from 'vitest'
import type { AgentDetail } from '../api/endpoints/agents'
import type { ModelSummary } from '../api/endpoints/catalogue'
import { EMBEDDING_DEFAULT_MODELS, EMBEDDING_PROVIDERS, IMAGE_GEN_DEFAULT_MODELS, IMAGE_GEN_PROVIDERS, LOCAL_EMBEDDING_DEFAULT, LOCAL_EMBEDDING_MODELS, STT_DEFAULT_MODELS, STT_PROVIDERS, TTS_DEFAULT_MODELS, TTS_DEFAULT_VOICES, TTS_PROVIDERS, emptyForm, fromDetail, modelLimits, problems, refilledLimits, slugOf, toInput } from './agentForm'

const detail = (patch: Partial<AgentDetail> = {}): AgentDetail => ({
    agent_id: 'archivist',
    name: 'Archivist',
    provider: 'anthropic',
    model: 'claude-haiku-5-5',
    thinking_depth: 100,
    checkpoint_threshold: 0.8,
    fetch: true,
    http_client: false,
    prompt_timeout: '10m',
    dream_enabled: false,
    tools_timeout: '30m',
    silent_read_initiative_chance: 0,
    tts: true,
    tts_settings: { provider: 'elevenlabs', model: 'eleven_v3', voice: 'abc', speed: 1.2 },
    stt: false,
    read_image: false,
    image_gen: false,
    ...patch,
})

describe('slugOf', () => {
    it('makes an id from a name', () => {
        expect(slugOf('Meeting Notes!')).toBe('meeting-notes')
        expect(slugOf('Éclair  Bot')).toBe('eclair-bot')
        expect(slugOf('')).toBe('')
    })
})

describe('problems', () => {
    it('asks for what the Identity step needs', () => {
        const p = problems(emptyForm(), 'config')
        expect(Object.keys(p).sort()).toEqual(['agentId', 'name'])
    })

    it('does not ask for an id when editing', () => {
        const f = { ...emptyForm(), name: 'A', provider: 'x', model: 'y' }
        expect(problems(f, 'config', true)).toEqual({})
    })

    it('asks for the model and a vision model only when read image has its own', () => {
        const f = { ...emptyForm(), readImage: true }
        expect(Object.keys(problems(f, 'models')).sort()).toEqual(['model', 'provider'])
        expect(Object.keys(problems({ ...f, visionOwn: true }, 'models')).sort()).toEqual(['model', 'provider', 'vision'])
    })

    it('asks for a dream schedule only while dreams are on', () => {
        const f = { ...emptyForm(), provider: 'x', model: 'y' }
        expect(problems(f, 'models')).toEqual({})
        // The form seeds a schedule, so only a cleared one is a problem.
        expect(problems({ ...f, dreamEnabled: true, dreamSchedule: '  ' }, 'models')).toEqual({ dreamSchedule: 'A cron expression is needed when dreams are on.' })
    })

    it('asks for the embedding model on Models, where its card is', () => {
        const f = { ...emptyForm(), provider: 'x', model: 'y' }
        // The built-in local index and Bedrock both have a documented default.
        expect(problems(f, 'models')).toEqual({})
        expect(problems({ ...f, embeddingProvider: 'bedrock' }, 'models')).toEqual({})
        expect(problems({ ...f, embeddingProvider: 'openai' }, 'models')).toEqual({ embeddingModel: 'Name the embedding model.' })
    })
})

describe('toInput', () => {
    it('round-trips an agent without losing settings the form has no field for', () => {
        const d = detail()
        const input = toInput(fromDetail(d), d)
        expect(input.agent_id).toBe('archivist')
        expect(input.checkpoint_threshold).toBe(0.8)
        expect(input.tools?.tts_settings).toEqual({ provider: 'elevenlabs', model: 'eleven_v3', voice: 'abc', speed: 1.2 })
    })

    it('resets a voice and model when the provider changes', () => {
        const d = detail()
        // The form clears them when the provider is switched.
        const f = { ...fromDetail(d), ttsProvider: 'openai', ttsModel: '', ttsVoice: '' }
        expect(toInput(f, d).tools?.tts_settings).toMatchObject({ provider: 'openai', model: null, voice: null })
    })

    it('sends the main model for images when read image has no model of its own', () => {
        const f = { ...emptyForm(), readImage: true, visionProvider: 'x', visionModel: 'y' }
        expect(toInput(f).tools?.read_image_settings).toEqual({ provider: null, model: null })
        expect(toInput({ ...f, visionOwn: true }).tools?.read_image_settings).toEqual({ provider: 'x', model: 'y' })
    })

    it('carries the full speech and image settings', () => {
        const f = { ...emptyForm(), stt: true, sttProvider: 'openai', sttModel: 'm', sttLanguage: 'en', imageGen: true, imageGenSize: '512x512' }
        expect(toInput(f).tools?.stt_settings).toMatchObject({ provider: 'openai', model: 'm', language: 'en' })
        expect(toInput(f).tools?.image_gen_settings).toMatchObject({ model: null, size: '512x512' })
    })

    it('leaves shell and HTTP to their addons', () => {
        const tools = toInput(emptyForm()).tools
        expect(tools).not.toHaveProperty('shell')
        expect(tools).not.toHaveProperty('fetch')
        expect(tools).not.toHaveProperty('http_client')
    })
})

describe('embedding', () => {
    it('sends none without a model, and wants one for a remote provider', () => {
        expect(toInput(emptyForm()).embedding).toBeNull()
        expect(toInput({ ...emptyForm(), embeddingModel: 'm' }).embedding).toEqual({ provider: 'local', model: 'm' })
        expect(problems({ ...emptyForm(), embeddingProvider: 'openai' }, 'all').embeddingModel).toBeTruthy()
        expect(problems(emptyForm(), 'config').embeddingModel).toBeUndefined()
    })

    it('accepts an empty Bedrock model, which selects Titan Text Embeddings V2', () => {
        const f = { ...emptyForm(), embeddingProvider: 'bedrock' }
        expect(problems(f, 'all').embeddingModel).toBeUndefined()
        expect(toInput(f).embedding).toEqual({ provider: 'bedrock', model: '' })
        expect(EMBEDDING_PROVIDERS).toContain('bedrock')
        expect(EMBEDDING_DEFAULT_MODELS.bedrock).toBe('amazon.titan-embed-text-v2:0')
        // A typed model is stored unchanged.
        expect(toInput({ ...f, embeddingModel: 'cohere.embed-english-v3' }).embedding).toEqual({
            provider: 'bedrock',
            model: 'cohere.embed-english-v3',
        })
    })
})

describe('media providers', () => {
    it('gives every listed speech-to-text and image provider a default model', () => {
        for (const provider of STT_PROVIDERS) expect(STT_DEFAULT_MODELS[provider], `stt ${provider}`).toBeTruthy()
        for (const provider of IMAGE_GEN_PROVIDERS) expect(IMAGE_GEN_DEFAULT_MODELS[provider], `image ${provider}`).toBeTruthy()
        expect(IMAGE_GEN_PROVIDERS).toContain('bedrock')
        expect(IMAGE_GEN_DEFAULT_MODELS.bedrock).toBe('amazon.nova-canvas-v1:0')
    })

    it('gives every text-to-speech provider a default model and voice', () => {
        for (const provider of TTS_PROVIDERS) {
            expect(TTS_DEFAULT_MODELS[provider], `tts model ${provider}`).toBeTruthy()
            expect(TTS_DEFAULT_VOICES[provider], `tts voice ${provider}`).toBeTruthy()
        }
        // `alloy` is an OpenAI voice; Kokoro publishes `af_alloy`.
        expect(TTS_DEFAULT_MODELS.openrouter).toBe('hexgrad/kokoro-82m')
        expect(TTS_DEFAULT_VOICES.openrouter).toBe('af_alloy')
        expect(TTS_DEFAULT_VOICES.openai).toBe('alloy')
    })

    it('round-trips OpenRouter with every override empty', () => {
        const f = { ...emptyForm(), stt: true, sttProvider: 'openrouter', imageGen: true, imageGenProvider: 'openrouter' }
        const tools = toInput(f).tools
        expect(tools?.stt_settings).toMatchObject({ provider: 'openrouter', model: null, language: null })
        expect(tools?.image_gen_settings).toMatchObject({ provider: 'openrouter', model: null, size: null })
    })

    it('reads an OpenRouter setting back into the form', () => {
        const d = detail({
            stt: true,
            stt_settings: { provider: 'openrouter', model: null, language: null },
            image_gen: true,
            image_gen_settings: { provider: 'openrouter', model: null, size: null },
        })
        const f = fromDetail(d)
        expect(f.sttProvider).toBe('openrouter')
        expect(f.sttModel).toBe('')
        expect(f.imageGenProvider).toBe('openrouter')
        expect(f.imageGenModel).toBe('')
    })
})

const model = (limit: ModelSummary['limit'] = { context: 200_000, input: 200_000, output: 64_000 }): ModelSummary => ({
    id: 'claude-haiku-5-5',
    name: 'Claude Haiku 5.5',
    default: true,
    limit,
    caps: {},
    status: 'active',
})

describe('model limits', () => {
    it('takes the context window from input, falling back to context', () => {
        expect(modelLimits(model())).toEqual({ contextWindow: '200000', maxTokens: '64000' })
        expect(modelLimits(model({ context: 32_000, output: 8_000 }))).toEqual({ contextWindow: '32000', maxTokens: '8000' })
    })

    it('has nothing for a model the catalogue does not know', () => {
        expect(modelLimits(undefined)).toEqual({ contextWindow: '', maxTokens: '' })
        expect(modelLimits(model({}))).toEqual({ contextWindow: '', maxTokens: '' })
    })

    it('fills empty fields when a model is chosen', () => {
        const f = { ...emptyForm() }
        expect(refilledLimits(f, { contextWindow: '', maxTokens: '' }, model())).toEqual({ contextWindow: '200000', maxTokens: '64000' })
    })

    it('replaces what it filled in, but never a number someone typed', () => {
        const auto = { contextWindow: '200000', maxTokens: '64000' }
        expect(refilledLimits({ ...emptyForm(), contextWindow: '200000', maxTokens: '64000' }, auto, model({ input: 100_000, output: 16_000 }))).toEqual({
            contextWindow: '100000',
            maxTokens: '16000',
        })
        expect(refilledLimits({ ...emptyForm(), contextWindow: '90000', maxTokens: '64000' }, auto, model({ input: 100_000, output: 16_000 }))).toEqual({
            contextWindow: '90000',
            maxTokens: '16000',
        })
    })

    it('clears a filled value when the next model does not say', () => {
        expect(refilledLimits({ ...emptyForm(), contextWindow: '200000', maxTokens: '64000' }, { contextWindow: '200000', maxTokens: '64000' }, undefined)).toEqual({
            contextWindow: '',
            maxTokens: '',
        })
    })
})

describe('the dream model follows the language model', () => {
    it('is sent as none while it is the language model, so it keeps following it', () => {
        const f = { ...emptyForm(), provider: 'anthropic', model: 'claude-haiku-5-5', dreamProvider: 'anthropic', dreamModel: 'claude-haiku-5-5' }
        expect(toInput(f).dream_provider).toBeNull()
        expect(toInput(f).dream_model).toBeNull()
    })

    it('is sent as chosen once it is a model of its own', () => {
        const f = { ...emptyForm(), provider: 'anthropic', model: 'claude-haiku-5-5', dreamProvider: 'openai', dreamModel: 'gpt-5-mini' }
        expect(toInput(f).dream_provider).toBe('openai')
        expect(toInput(f).dream_model).toBe('gpt-5-mini')
    })
})

describe('local embedding models', () => {
    it('lists every model the server offers, once, with the default first', () => {
        expect(LOCAL_EMBEDDING_MODELS).toHaveLength(30)
        expect(new Set(LOCAL_EMBEDDING_MODELS.map((m) => m.value)).size).toBe(LOCAL_EMBEDDING_MODELS.length)
        expect(LOCAL_EMBEDDING_MODELS[0].value).toBe(LOCAL_EMBEDDING_DEFAULT)
        expect(LOCAL_EMBEDDING_MODELS[0].label).toBe('all_mini_lml6_v2 · lightweight')
    })
})
