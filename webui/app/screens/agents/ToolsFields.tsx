import { Card, Checkbox, FilterTabs, Select, TextField, Textarea, ToggleChip, ToggleSection } from '@vizier/design'
import {
    IMAGE_GEN_DEFAULT_MODELS,
    IMAGE_GEN_PROVIDERS,
    KOKORO_MODELS,
    KOKORO_VOICES,
    STT_DEFAULT_MODELS,
    WHISPER_MODELS,
    STT_PROVIDERS,
    TTS_DEFAULT_MODELS,
    TTS_DEFAULT_VOICES,
    TTS_PROVIDERS,
    type AgentForm,
} from '../../lib/agentForm'
import { useCan, PERMISSIONS } from '../../lib/access'
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

/** The Tools step of the wizard and Config's Tools tab: what the agent may use beyond memory. */
export function ToolsFields({ form, errors, set }: Props) {
    const canShell = useCan(PERMISSIONS.shellConfig)
    // One per capability, shared by its model and voice pickers. Mounting them makes no
    // request: each fetches when its picker is first opened (spec 028, FR-018/SC-011).
    const ttsMedia = useMediaModels('speech')
    const sttMedia = useMediaModels('transcription')
    const imageMedia = useMediaModels('image')

    return (
        <div className="flex flex-col gap-16">
            <Card title="Always on" description="Memory, Core, scheduling, skills, subtasks, delegation, session files and the scratchpad. Scripts reach every tool by category in code mode.">
                <div className="flex flex-wrap gap-8">
                    <ToggleChip label="Fetch Webpage" checked={form.fetch} onCheckedChange={(fetch) => set({ fetch })} />
                    <ToggleChip label="HTTP Client" checked={form.httpClient} onCheckedChange={(httpClient) => set({ httpClient })} />
                </div>
            </Card>

            <ToggleSection title="Text-to-Speech" description="Reply with a voice message." checked={form.tts} onCheckedChange={(tts) => set({ tts })}>
                <div className="grid grid-cols-1 gap-16 sm:grid-cols-2">
                    <Select
                        label="Provider"
                        options={options(TTS_PROVIDERS)}
                        value={form.ttsProvider}
                        onChange={(ttsProvider) => set({ ttsProvider, ttsModel: '', ttsVoice: '' })}
                    />
                    {form.ttsProvider === 'openrouter' ? (
                        <MediaModelSelect
                            capability="speech"
                            model={form.ttsModel}
                            fallbackDefault={TTS_DEFAULT_MODELS.openrouter}
                            media={ttsMedia}
                            onChange={(ttsModel) => set({ ttsModel })}
                        />
                    ) : form.ttsProvider === 'kokoro' ? (
                        <Select
                            label="Model"
                            mono
                            options={withCurrent(KOKORO_MODELS, form.ttsModel)}
                            value={form.ttsModel || TTS_DEFAULT_MODELS.kokoro}
                            onChange={(ttsModel) => set({ ttsModel })}
                        />
                    ) : (
                        <TextField label="Model" mono value={form.ttsModel} placeholder={TTS_DEFAULT_MODELS[form.ttsProvider] ?? 'Provider default'} onChange={(e) => set({ ttsModel: e.target.value })} />
                    )}
                    {form.ttsProvider === 'openrouter' ? (
                        <MediaVoiceSelect
                            model={form.ttsModel}
                            voice={form.ttsVoice}
                            fallbackDefault={TTS_DEFAULT_VOICES.openrouter}
                            media={ttsMedia}
                            onChange={(ttsVoice) => set({ ttsVoice })}
                        />
                    ) : form.ttsProvider === 'kokoro' ? (
                        <Select
                            label="Voice"
                            mono
                            options={withCurrent(KOKORO_VOICES, form.ttsVoice)}
                            value={form.ttsVoice && form.ttsVoice !== 'af' ? form.ttsVoice : 'af_heart'}
                            onChange={(ttsVoice) => set({ ttsVoice })}
                        />
                    ) : (
                        <TextField
                            label="Voice"
                            mono
                            value={form.ttsVoice}
                            placeholder={TTS_DEFAULT_VOICES[form.ttsProvider] ?? 'Provider default'}
                            onChange={(e) => set({ ttsVoice: e.target.value })}
                        />
                    )}
                    <TextField
                        label="Speed"
                        inputMode="decimal"
                        value={form.ttsSpeed}
                        placeholder="1.0"
                        invalid={!!errors.ttsSpeed}
                        helper={errors.ttsSpeed}
                        onChange={(e) => set({ ttsSpeed: e.target.value })}
                    />
                </div>
            </ToggleSection>

            <ToggleSection title="Speech-to-Text" description="Understand voice messages." checked={form.stt} onCheckedChange={(stt) => set({ stt })}>
                <div className="grid grid-cols-1 gap-16 sm:grid-cols-2">
                    <Select
                        label="Provider"
                        options={options(STT_PROVIDERS)}
                        value={form.sttProvider}
                        onChange={(sttProvider) => set({ sttProvider, sttModel: '' })}
                    />
                    {form.sttProvider === 'openrouter' ? (
                        <MediaModelSelect
                            capability="transcription"
                            model={form.sttModel}
                            fallbackDefault={STT_DEFAULT_MODELS.openrouter}
                            media={sttMedia}
                            onChange={(sttModel) => set({ sttModel })}
                        />
                    ) : form.sttProvider === 'whisper' ? (
                        <Select
                            label="Model"
                            mono
                            options={withCurrent(WHISPER_MODELS, form.sttModel)}
                            value={form.sttModel || STT_DEFAULT_MODELS.whisper}
                            onChange={(sttModel) => set({ sttModel })}
                        />
                    ) : (
                        <TextField
                            label="Model"
                            mono
                            value={form.sttModel}
                            placeholder={STT_DEFAULT_MODELS[form.sttProvider] ?? 'Provider default'}
                            onChange={(e) => set({ sttModel: e.target.value })}
                        />
                    )}
                    <TextField
                        label="Language"
                        mono
                        value={form.sttLanguage}
                        placeholder="Detect automatically"
                        helper="A language code such as en."
                        onChange={(e) => set({ sttLanguage: e.target.value })}
                    />
                </div>
            </ToggleSection>

            <ToggleSection
                title="Read Image"
                description="Describe pictures with a vision model when the main model can't."
                checked={form.readImage}
                onCheckedChange={(readImage) => set({ readImage })}
            >
                <div className="flex flex-col gap-12">
                    <p className="type-small text-faint">Pictures go straight to the main model.</p>
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
                    <Select
                        label="Provider"
                        options={options(IMAGE_GEN_PROVIDERS)}
                        value={form.imageGenProvider}
                        onChange={(imageGenProvider) => set({ imageGenProvider, imageGenModel: '' })}
                    />
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

            <ToggleSection
                title="Shell"
                description={canShell ? 'Run commands on this machine or in a container.' : 'Setting up a shell needs the shell permission.'}
                checked={form.shell}
                disabled={!canShell && !form.shell}
                onCheckedChange={(shell) => set({ shell })}
            >
                <div className="flex flex-col gap-16">
                    <FilterTabs
                        aria-label="Shell environment"
                        options={[
                            { value: 'local', label: 'Local' },
                            { value: 'docker', label: 'Docker' },
                        ]}
                        value={form.shellKind}
                        onChange={(shellKind) => set({ shellKind })}
                    />
                    {form.shellKind === 'local' ? (
                        <TextField
                            label="Working directory"
                            mono
                            value={form.shellPath}
                            invalid={!!errors.shellPath}
                            helper={errors.shellPath}
                            placeholder="/home/you/work"
                            onChange={(e) => set({ shellPath: e.target.value })}
                        />
                    ) : (
                        <>
                            <FilterTabs
                                aria-label="Image source"
                                options={[
                                    { value: 'pull', label: 'Pull an image' },
                                    { value: 'dockerfile', label: 'Build a Dockerfile' },
                                ]}
                                value={form.dockerSource}
                                onChange={(dockerSource) => set({ dockerSource })}
                            />
                            {form.dockerSource === 'dockerfile' && (
                                <TextField
                                    label="Dockerfile"
                                    mono
                                    value={form.dockerfilePath}
                                    invalid={!!errors.dockerfilePath}
                                    helper={errors.dockerfilePath}
                                    onChange={(e) => set({ dockerfilePath: e.target.value })}
                                />
                            )}
                            <TextField
                                label={form.dockerSource === 'pull' ? 'Image' : 'Image name'}
                                mono
                                value={form.dockerImage}
                                invalid={!!errors.dockerImage}
                                helper={errors.dockerImage}
                                onChange={(e) => set({ dockerImage: e.target.value })}
                            />
                            <TextField label="Container name" mono value={form.containerName} onChange={(e) => set({ containerName: e.target.value })} />
                        </>
                    )}
                    <Textarea
                        label="Environment"
                        mono
                        rows={4}
                        helper="One KEY=value per line."
                        value={form.shellEnv}
                        onChange={(e) => set({ shellEnv: e.target.value })}
                    />
                </div>
            </ToggleSection>
        </div>
    )
}
