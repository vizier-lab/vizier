import { Button, Card, FilterTabs, ThemeScope } from '@vizier/design'
import { useEffect } from 'react'
import { allVars, appearanceOf } from '../lib/themes/apply'
import { DEFAULT_SCHEME_ID, type Mode } from '../lib/themes/roles'
import { PresetGrid } from '../screens/appearance/PresetGrid'
import { SchemePreview } from '../screens/appearance/SchemePreview'
import { SettingsFrame } from '../screens/shared/SettingsFrame'
import { useAppearance } from '../stores/appearance'

const MODES: { value: Mode; label: string }[] = [
    { value: 'system', label: 'System' },
    { value: 'light', label: 'Light' },
    { value: 'dark', label: 'Dark' },
]

export default function SettingsAppearance() {
    const { schemeId, mode, deviceIsDark, presets, load, select, setMode, useDefault } = useAppearance()

    useEffect(() => void load(), [load])

    const scheme = presets.find((s) => s.id === schemeId)
    const vizier = presets.find((s) => s.id === DEFAULT_SCHEME_ID)
    const appearance = scheme && appearanceOf(scheme, mode, deviceIsDark)

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
                            {/* In Vizier's own colours, so it can be read and pressed whatever is on screen (FR-017). */}
                            <ThemeScope vars={allVars(vizier[deviceIsDark ? 'dark' : 'light']!.roles)} appearance={deviceIsDark ? 'dark' : 'light'} className="inline-flex rounded-md bg-panel">
                                <Button variant="secondary" onClick={useDefault}>
                                    Use default
                                </Button>
                            </ThemeScope>
                        </div>
                    </Card>
                    <Card title="Preview">
                        <SchemePreview variant={scheme[appearance]!} appearance={appearance} />
                    </Card>
                </div>
            )}
        </SettingsFrame>
    )
}
