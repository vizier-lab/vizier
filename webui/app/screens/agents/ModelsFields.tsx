import { Card, Checkbox, Select, TextField, ToggleSection } from '@vizier/design'
import { useRef } from 'react'
import type { ModelSummary } from '../../api/endpoints/catalogue'
import {
    EMBEDDING_DEFAULT_MODELS,
    EMBEDDING_PROVIDERS,
    IMAGE_GEN_DEFAULT_MODELS,
    IMAGE_GEN_PROVIDERS,
    KOKORO_MODELS,
    KOKORO_VOICES,
    LOCAL_EMBEDDING_DEFAULT,
    LOCAL_EMBEDDING_MODELS,
    STT_DEFAULT_MODELS,
    WHISPER_MODELS,
    STT_PROVIDERS,
    TTS_DEFAULT_MODELS,
    TTS_DEFAULT_VOICES,
    TTS_PROVIDERS,
    modelLimits,
    refilledLimits,
    type AgentForm,
} from '../../lib/agentForm'
import { MediaModelSelect } from './MediaModelSelect'
import { MediaVoiceSelect } from './MediaVoiceSelect'
import { ModelSelect } from './ModelSelect'
import { useMediaModels } from './useMediaModels'

type Props = {
    form: AgentForm
    errors: Record<string, string>
    set: (patch: Partial<AgentForm>) => void
}

const options = (values: string[]) => values.map((v) => ({ value: v, label: v }))

/** The list, plus the saved value when it is not in it (an older name stays visible). */
const withCurrent = (list: { value: string; label: string }[], current: string) =>
    current && !list.some((o) => o.value === current) ? [{ value: current, label: current }, ...list] : list

/**
 * Every model an agent can use, in one section (spec 040): the language model and its limits,
 * the dream cycle with its schedule and model, voice and pictures, and the memory index's embeddings.
 */
export function ModelsFields({ form, errors, set }: Props) {
    // One per capability, shared by its model and voice pickers. Mounting them makes no
    // request: each fetches when its picker is first opened (spec 028, FR-018/SC-011).
    const ttsMedia = useMediaModels('speech')
    const sttMedia = useMediaModels('transcription')
    const imageMedia = useMediaModels('image')

    // The limits the catalogue gave for the last chosen model. A model change refills them, but a
    // number someone typed is theirs and stays: only an empty field or one we filled is replaced.
    const limitsFilled = useRef({ contextWindow: '', maxTokens: '' })
    // The language model the dream cycle and the vision reader are following.
    const followed = useRef({ provider: '', model: '' })
    const pickModel = (choice: { provider: string; model: string; info?: ModelSummary }) => {
        const patch: Partial<AgentForm> = {
            provider: choice.provider,
            model: choice.model,
            ...refilledLimits(form, limitsFilled.current, choice.info),
        }
        limitsFilled.current = modelLimits(choice.info)

        // The dream cycle and, when it can read pictures, the vision reader follow the language
        // model until someone gives them one of their own: only blanks and last time's default move.
        const follows = (provider: string, model: string) =>
            (!provider || provider === followed.current.provider) && (!model || model === followed.current.model)
        if (follows(form.dreamProvider, form.dreamModel)) {
            patch.dreamProvider = choice.provider
            patch.dreamModel = choice.model
        }
        if (choice.info?.caps.image_input === true && follows(form.visionProvider, form.visionModel)) {
            patch.visionProvider = choice.provider
            patch.visionModel = choice.model
        }
        followed.current = { provider: choice.provider, model: choice.model }
        set(patch)
    }

    return (
        <div className="flex flex-col gap-16">
            <Card title="Language model" description="From the models.dev catalogue and your custom providers. It carries every turn unless a capability below uses its own.">
                <div className="flex flex-col gap-16">
                    <ModelSelect provider={form.provider} model={form.model} onChange={pickModel} />
                    {(errors.provider || errors.model) && <p className="type-small text-failed-text">{errors.provider ?? errors.model}</p>}
                    <div className="grid grid-cols-1 gap-16 sm:grid-cols-3">
                        <TextField
                            label="Context window"
                            mono
                            suffix="tokens"
                            inputMode="numeric"
                            value={form.contextWindow}
                            invalid={!!errors.contextWindow}
                            helper={errors.contextWindow ?? 'Filled from the model; empty uses its own.'}
                            onChange={(e) => set({ contextWindow: e.target.value })}
                        />
                        <TextField
                            label="Max output"
                            mono
                            suffix="tokens"
                            inputMode="numeric"
                            value={form.maxTokens}
                            invalid={!!errors.maxTokens}
                            helper={errors.maxTokens ?? 'Filled from the model; empty uses its own.'}
                            onChange={(e) => set({ maxTokens: e.target.value })}
                        />
                        <TextField
                            label="Thinking depth"
                            mono
                            suffix="steps"
                            inputMode="numeric"
                            value={form.thinkingDepth}
                            helper="Most tool rounds in one turn."
                            onChange={(e) => set({ thinkingDepth: e.target.value })}
                        />
                    </div>
                </div>
            </Card>

            <ToggleSection
                title="Dreams"
                description="Reflect and tidy memory on a schedule. It thinks with the language model unless you give it one of its own."
                checked={form.dreamEnabled}
                onCheckedChange={(dreamEnabled) => set({ dreamEnabled })}
            >
                <div className="flex flex-col gap-16">
                    <TextField
                        label="Dream schedule"
                        mono
                        value={form.dreamSchedule}
                        invalid={!!errors.dreamSchedule}
                        helper={errors.dreamSchedule ?? 'A cron expression, e.g. 0 3 * * *.'}
                        onChange={(e) => set({ dreamSchedule: e.target.value })}
                    />
                    <ModelSelect
                        compact
                        providerLabel="Dream provider"
                        modelLabel="Dream model"
                        provider={form.dreamProvider || form.provider}
                        model={form.dreamModel || form.model}
                        onChange={(c) => set({ dreamProvider: c.provider, dreamModel: c.model })}
                    />
                </div>
            </ToggleSection>

            <Card title="Voice" description="Hear and answer voice messages. Both are off until you switch them on.">
                <div className="flex flex-col gap-20">
                    <ToggleSection title="Text-to-Speech" description="Reply with a voice message." checked={form.tts} onCheckedChange={(tts) => set({ tts })}>
                        <div className="grid grid-cols-1 gap-16 sm:grid-cols-2">
                            <Select
                                label="Provider"
                                options={options(TTS_PROVIDERS)}
                                value={form.ttsProvider}
                                onChange={(ttsProvider) => set({ ttsProvider, ttsModel: '', ttsVoice: '' })}
                            />
                            {form.ttsProvider === 'openrouter' ? (
                                <MediaModelSelect capability="speech" model={form.ttsModel} fallbackDefault={TTS_DEFAULT_MODELS.openrouter} media={ttsMedia} onChange={(ttsModel) => set({ ttsModel })} />
                            ) : form.ttsProvider === 'kokoro' ? (
                                <Select label="Model" mono options={withCurrent(KOKORO_MODELS, form.ttsModel)} value={form.ttsModel || TTS_DEFAULT_MODELS.kokoro} onChange={(ttsModel) => set({ ttsModel })} />
                            ) : (
                                <TextField label="Model" mono value={form.ttsModel} placeholder={TTS_DEFAULT_MODELS[form.ttsProvider] ?? 'Provider default'} onChange={(e) => set({ ttsModel: e.target.value })} />
                            )}
                            {form.ttsProvider === 'openrouter' ? (
                                <MediaVoiceSelect model={form.ttsModel} voice={form.ttsVoice} fallbackDefault={TTS_DEFAULT_VOICES.openrouter} media={ttsMedia} onChange={(ttsVoice) => set({ ttsVoice })} />
                            ) : form.ttsProvider === 'kokoro' ? (
                                <Select
                                    label="Voice"
                                    mono
                                    options={withCurrent(KOKORO_VOICES, form.ttsVoice)}
                                    value={form.ttsVoice && form.ttsVoice !== 'af' ? form.ttsVoice : 'af_heart'}
                                    onChange={(ttsVoice) => set({ ttsVoice })}
                                />
                            ) : (
                                <TextField label="Voice" mono value={form.ttsVoice} placeholder={TTS_DEFAULT_VOICES[form.ttsProvider] ?? 'Provider default'} onChange={(e) => set({ ttsVoice: e.target.value })} />
                            )}
                            <TextField label="Speed" inputMode="decimal" value={form.ttsSpeed} placeholder="1.0" invalid={!!errors.ttsSpeed} helper={errors.ttsSpeed} onChange={(e) => set({ ttsSpeed: e.target.value })} />
                        </div>
                    </ToggleSection>

                    <ToggleSection title="Speech-to-Text" description="Understand voice messages." checked={form.stt} onCheckedChange={(stt) => set({ stt })}>
                        <div className="grid grid-cols-1 gap-16 sm:grid-cols-2">
                            <Select label="Provider" options={options(STT_PROVIDERS)} value={form.sttProvider} onChange={(sttProvider) => set({ sttProvider, sttModel: '' })} />
                            {form.sttProvider === 'openrouter' ? (
                                <MediaModelSelect
                                    capability="transcription"
                                    model={form.sttModel}
                                    fallbackDefault={STT_DEFAULT_MODELS.openrouter}
                                    media={sttMedia}
                                    onChange={(sttModel) => set({ sttModel })}
                                />
                            ) : form.sttProvider === 'whisper' ? (
                                <Select label="Model" mono options={withCurrent(WHISPER_MODELS, form.sttModel)} value={form.sttModel || STT_DEFAULT_MODELS.whisper} onChange={(sttModel) => set({ sttModel })} />
                            ) : (
                                <TextField label="Model" mono value={form.sttModel} placeholder={STT_DEFAULT_MODELS[form.sttProvider] ?? 'Provider default'} onChange={(e) => set({ sttModel: e.target.value })} />
                            )}
                            <TextField label="Language" mono value={form.sttLanguage} placeholder="Detect automatically" helper="A language code such as en." onChange={(e) => set({ sttLanguage: e.target.value })} />
                        </div>
                    </ToggleSection>
                </div>
            </Card>

            <Card title="Pictures" description="Read images and draw them. Both are off until you switch them on.">
                <div className="flex flex-col gap-20">
                    <ToggleSection title="Read Image" description="Describe pictures with a vision model when the main model can't." checked={form.readImage} onCheckedChange={(readImage) => set({ readImage })}>
                        <div className="flex flex-col gap-12">
                            <p className="type-small text-faint">With no model of its own, pictures go straight to the language model.</p>
                            <Checkbox label="Use a different model" checked={form.visionOwn} onChange={(e) => set({ visionOwn: e.target.checked })} />
                            {form.visionOwn && (
                                <ModelSelect
                                    needsImages
                                    provider={form.visionProvider}
                                    model={form.visionModel}
                                    providerLabel="Vision provider"
                                    modelLabel="Vision model"
                                    onChange={(c) => set({ visionProvider: c.provider, visionModel: c.model })}
                                />
                            )}
                            {errors.vision && <p className="type-small text-failed-text">{errors.vision}</p>}
                        </div>
                    </ToggleSection>

                    <ToggleSection title="Image Generation" description="Draw pictures on request." checked={form.imageGen} onCheckedChange={(imageGen) => set({ imageGen })}>
                        <div className="grid grid-cols-1 gap-16 sm:grid-cols-2">
                            <Select label="Provider" options={options(IMAGE_GEN_PROVIDERS)} value={form.imageGenProvider} onChange={(imageGenProvider) => set({ imageGenProvider, imageGenModel: '' })} />
                            {form.imageGenProvider === 'openrouter' ? (
                                <MediaModelSelect
                                    capability="image"
                                    model={form.imageGenModel}
                                    fallbackDefault={IMAGE_GEN_DEFAULT_MODELS.openrouter}
                                    media={imageMedia}
                                    onChange={(imageGenModel) => set({ imageGenModel })}
                                />
                            ) : (
                                <TextField
                                    label="Model"
                                    mono
                                    value={form.imageGenModel}
                                    placeholder={IMAGE_GEN_DEFAULT_MODELS[form.imageGenProvider] ?? 'Provider default'}
                                    onChange={(e) => set({ imageGenModel: e.target.value })}
                                />
                            )}
                            <TextField label="Size" mono value={form.imageGenSize} placeholder="Provider default" helper="Such as 1024x1024." onChange={(e) => set({ imageGenSize: e.target.value })} />
                        </div>
                    </ToggleSection>
                </div>
            </Card>

            <Card title="Memory index" description="Embeddings let the agent find passages from its memory. Changing them re-indexes memory when the agent starts.">
                <div className="flex flex-col gap-16">
                    <Select label="Indexer" options={[{ value: 'sqlite', label: 'SQLite (vector store)' }]} value="sqlite" onChange={() => {}} disabled />
                    <Select
                        label="Embedding provider"
                        options={EMBEDDING_PROVIDERS.map((p) => ({ value: p, label: p }))}
                        value={form.embeddingProvider}
                        onChange={(embeddingProvider) => set({ embeddingProvider, embeddingModel: '' })}
                    />
                    {form.embeddingProvider === 'local' ? (
                        <Select
                            label="Embedding model"
                            mono
                            options={[{ value: '', label: `Built-in · ${LOCAL_EMBEDDING_DEFAULT}` }, ...withCurrent(LOCAL_EMBEDDING_MODELS, form.embeddingModel)]}
                            value={form.embeddingModel}
                            invalid={!!errors.embeddingModel}
                            helper={errors.embeddingModel ?? 'Empty uses the built-in model, downloaded once on first start.'}
                            onChange={(embeddingModel) => set({ embeddingModel })}
                        />
                    ) : (
                        <TextField
                            label="Embedding model"
                            mono
                            value={form.embeddingModel}
                            invalid={!!errors.embeddingModel}
                            placeholder={EMBEDDING_DEFAULT_MODELS[form.embeddingProvider]}
                            helper={
                                errors.embeddingModel ??
                                (form.embeddingProvider === 'bedrock'
                                    ? 'Empty uses Titan Text Embeddings V2.'
                                    : 'Changing it re-indexes memory when the agent starts.')
                            }
                            onChange={(e) => set({ embeddingModel: e.target.value })}
                        />
                    )}
                </div>
            </Card>
        </div>
    )
}
