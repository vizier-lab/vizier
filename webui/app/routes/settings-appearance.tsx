import { Button, Card, FilterTabs, ThemeScope } from '@vizier/design'
import { useEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router'
import { transport } from '../api/transport'
import { allVars, appearanceOf } from '../lib/themes/apply'
import { fileNameFor, parse, serialize } from '../lib/themes/file'
import { DEFAULT_SCHEME_ID, type Mode } from '../lib/themes/roles'
import { PresetGrid } from '../screens/appearance/PresetGrid'
import { SchemePreview } from '../screens/appearance/SchemePreview'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { SettingsFrame } from '../screens/shared/SettingsFrame'
import { useAppearance } from '../stores/appearance'
import { toast } from '../stores/toasts'

const MODES: { value: Mode; label: string }[] = [
    { value: 'system', label: 'System' },
    { value: 'light', label: 'Light' },
    { value: 'dark', label: 'Dark' },
]

export default function SettingsAppearance() {
    const navigate = useNavigate()
    const { schemeId, mode, deviceIsDark, presets, customs, load, select, setMode, useDefault, saveCustom, deleteCustom } = useAppearance()
    const [deleting, setDeleting] = useState(false)
    const importer = useRef<HTMLInputElement>(null)

    useEffect(() => void load(), [load])

    const all = [...presets, ...customs]
    const scheme = all.find((s) => s.id === schemeId)
    const vizier = presets.find((s) => s.id === DEFAULT_SCHEME_ID)
    const appearance = scheme && appearanceOf(scheme, mode, deviceIsDark)
    const own = scheme?.kind === 'custom'

    const exportScheme = () => {
        if (!scheme) return
        transport.saveText(serialize(scheme), fileNameFor(scheme.name)).catch(() => toast.error("That theme couldn't be exported."))
    }

    const importFile = async (file: File) => {
        const result = parse(await file.text(), all)
        if (!result.ok) {
            toast.error("That theme couldn't be imported", result.error)
            return
        }
        const problem = saveCustom(result.scheme)
        if (problem) toast.error("That theme couldn't be imported", problem)
        else toast.success(`Imported ${result.scheme.name}`, 'Select it under Your themes to use it.')
    }

    return (
        <SettingsFrame title="Appearance" description="How Vizier looks on this device. Nothing here is sent to the server.">
            {scheme && vizier && appearance && (
                <div className="flex flex-col gap-16">
                    <Card
                        title="Theme"
                        headerTrailing={
                            scheme.light && scheme.dark ? (
                                <FilterTabs aria-label="Light or dark" options={MODES} value={mode} onChange={setMode} />
                            ) : (
                                <span className="type-small text-faint">Always {scheme.dark ? 'dark' : 'light'}</span>
                            )
                        }
                    >
                        <PresetGrid schemes={presets} selectedId={schemeId} mode={mode} deviceIsDark={deviceIsDark} onSelect={select} />
                        <div className="flex flex-wrap gap-8">
                            <Button variant="secondary" icon="copy" onClick={() => navigate(`/settings/appearance/new?from=${encodeURIComponent(scheme.id)}`)}>
                                Duplicate
                            </Button>
                            <Button variant="secondary" icon="download" onClick={exportScheme}>
                                Export
                            </Button>
                            {/* In Vizier's own colours, so it can be read and pressed whatever is on screen (FR-017). */}
                            <ThemeScope vars={allVars(vizier[deviceIsDark ? 'dark' : 'light']!.roles)} appearance={deviceIsDark ? 'dark' : 'light'} className="inline-flex rounded-md bg-panel">
                                <Button variant="secondary" onClick={useDefault}>
                                    Use default
                                </Button>
                            </ThemeScope>
                        </div>
                    </Card>
                    <Card
                        title="Your themes"
                        headerTrailing={
                            <>
                                <input
                                    ref={importer}
                                    type="file"
                                    accept=".json,application/json"
                                    hidden
                                    onChange={(e) => {
                                        const file = e.target.files?.[0]
                                        e.target.value = ''
                                        if (file) void importFile(file)
                                    }}
                                />
                                <Button variant="secondary" icon="upload" onClick={() => importer.current?.click()}>
                                    Import
                                </Button>
                            </>
                        }
                    >
                        {customs.length > 0 ? (
                            <>
                                <PresetGrid schemes={customs} selectedId={schemeId} mode={mode} deviceIsDark={deviceIsDark} onSelect={select} />
                                {own && (
                                    <div className="flex flex-wrap gap-8">
                                        <Button variant="secondary" icon="pencil" onClick={() => navigate(`/settings/appearance/${encodeURIComponent(scheme.id)}`)}>
                                            Edit
                                        </Button>
                                        <Button variant="secondary" icon="trash" onClick={() => setDeleting(true)}>
                                            Delete
                                        </Button>
                                    </div>
                                )}
                            </>
                        ) : (
                            <p className="type-ui-para text-muted">Duplicate a theme to edit it here, or export one as a template and import your version. Yours stay in this browser, so export the ones you'd miss.</p>
                        )}
                    </Card>
                    <Card title="Preview">
                        <SchemePreview variant={scheme[appearance]!} appearance={appearance} />
                    </Card>
                    <ConfirmDialog
                        open={deleting}
                        title={`Delete ${scheme.name}?`}
                        message="It is removed from this browser, and Vizier goes back to the default theme. Export it first if you may want it again."
                        confirmLabel="Delete"
                        onConfirm={() => {
                            setDeleting(false)
                            deleteCustom(scheme.id)
                        }}
                        onCancel={() => setDeleting(false)}
                    />
                </div>
            )}
        </SettingsFrame>
    )
}
