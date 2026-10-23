import { Card, FilterTabs, InlineSelect, Select, TextField, Textarea, ToggleChip, ToggleSection } from '@vizier/design'
import { useState } from 'react'
import { IMAGE_GEN_PROVIDERS, STT_PROVIDERS, TTS_PROVIDERS, type AgentForm } from '../../lib/agentForm'
import { useCan, PERMISSIONS } from '../../lib/access'
import { ModelSelect } from './ModelSelect'

type Props = {
    form: AgentForm
    errors: Record<string, string>
    set: (patch: Partial<AgentForm>) => void
}

const options = (values: string[]) => values.map((v) => ({ value: v, label: v }))

/** The Tools step of the wizard and Config's Tools tab: what the agent may use beyond memory. */
export function ToolsFields({ form, errors, set }: Props) {
    const canShell = useCan(PERMISSIONS.shellConfig)
    const [visionOpen, setVisionOpen] = useState(false)

    return (
        <div className="flex flex-col gap-16">
            <Card title="Always on" description="Memory, Core, scheduling, skills, subtasks, delegation, session files and the scratchpad. Scripts reach every tool by category in code mode.">
                <div className="flex flex-wrap gap-8">
                    <ToggleChip label="Fetch Webpage" checked={form.fetch} onCheckedChange={(fetch) => set({ fetch })} />
                    <ToggleChip label="HTTP Client" checked={form.httpClient} onCheckedChange={(httpClient) => set({ httpClient })} />
                </div>
            </Card>

            <ToggleSection title="Text-to-Speech" description="Reply with a voice message." checked={form.tts} onCheckedChange={(tts) => set({ tts })}>
                <Select label="Provider" options={options(TTS_PROVIDERS)} value={form.ttsProvider} onChange={(ttsProvider) => set({ ttsProvider })} />
            </ToggleSection>

            <ToggleSection title="Speech-to-Text" description="Understand voice messages." checked={form.stt} onCheckedChange={(stt) => set({ stt })}>
                <Select label="Provider" options={options(STT_PROVIDERS)} value={form.sttProvider} onChange={(sttProvider) => set({ sttProvider })} />
            </ToggleSection>

            <ToggleSection
                title="Read Image"
                description="Describe pictures with a vision model when the main model can't."
                checked={form.readImage}
                onCheckedChange={(readImage) => set({ readImage })}
            >
                <div className="flex flex-col gap-8">
                    <InlineSelect
                        label="Vision model"
                        value={form.visionModel ? `${form.visionProvider} · ${form.visionModel}` : 'Choose…'}
                        open={visionOpen}
                        onClick={() => setVisionOpen((o) => !o)}
                    />
                    {visionOpen && (
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
                <Select
                    label="Provider"
                    options={options(IMAGE_GEN_PROVIDERS)}
                    value={form.imageGenProvider}
                    onChange={(imageGenProvider) => set({ imageGenProvider })}
                />
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
