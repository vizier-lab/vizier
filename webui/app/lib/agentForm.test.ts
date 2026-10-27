import { describe, expect, it } from 'vitest'
import type { AgentDetail } from '../api/endpoints/agents'
import { IMAGE_GEN_DEFAULT_MODELS, IMAGE_GEN_PROVIDERS, STT_DEFAULT_MODELS, STT_PROVIDERS, TTS_DEFAULT_MODELS, TTS_DEFAULT_VOICES, TTS_PROVIDERS, emptyForm, formatEnv, fromDetail, parseEnv, problems, slugOf, toInput } from './agentForm'

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

describe('env text', () => {
    it('round-trips KEY=value lines and skips comments', () => {
        expect(parseEnv('# note\nA=1\n\nB = two=2')).toEqual({ A: '1', B: 'two=2' })
        expect(formatEnv({ A: '1', B: '2' })).toBe('A=1\nB=2')
        expect(parseEnv('')).toBeNull()
    })
})

describe('problems', () => {
    it('asks for what the Config step needs', () => {
        const p = problems(emptyForm(), 'config')
        expect(Object.keys(p).sort()).toEqual(['agentId', 'model', 'name', 'provider'])
    })

    it('does not ask for an id when editing', () => {
        const f = { ...emptyForm(), name: 'A', provider: 'x', model: 'y' }
        expect(problems(f, 'config', true)).toEqual({})
    })

    it('needs a path for a local shell and a vision model only when read image has its own', () => {
        const f = { ...emptyForm(), shell: true, readImage: true }
        expect(Object.keys(problems(f, 'tools'))).toEqual(['shellPath'])
        expect(Object.keys(problems({ ...f, visionOwn: true }, 'tools')).sort()).toEqual(['shellPath', 'vision'])
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

    it('builds a docker shell from the form', () => {
        const f = { ...emptyForm(), shell: true, shellKind: 'docker' as const, dockerImage: 'node:22', shellEnv: 'A=1' }
        expect(toInput(f).tools?.shell).toEqual({
            environment: 'docker',
            image: { source: 'pull', name: 'node:22' },
            container_name: 'vizier',
            env: { A: '1' },
        })
    })
})

describe('embedding', () => {
    it('sends none without a model, and wants one for a remote provider', () => {
        expect(toInput(emptyForm()).embedding).toBeNull()
        expect(toInput({ ...emptyForm(), embeddingModel: 'm' }).embedding).toEqual({ provider: 'local', model: 'm' })
        expect(problems({ ...emptyForm(), embeddingProvider: 'openai' }, 'all').embeddingModel).toBeTruthy()
        expect(problems(emptyForm(), 'config').embeddingModel).toBeUndefined()
    })
})

describe('media providers', () => {
    it('gives every listed speech-to-text and image provider a default model', () => {
        for (const provider of STT_PROVIDERS) expect(STT_DEFAULT_MODELS[provider], `stt ${provider}`).toBeTruthy()
        for (const provider of IMAGE_GEN_PROVIDERS) expect(IMAGE_GEN_DEFAULT_MODELS[provider], `image ${provider}`).toBeTruthy()
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
