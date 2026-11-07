import { ThemeTile } from '@vizier/design'
import { appearanceOf } from '../../lib/themes/apply'
import { schemeDetail } from '../../lib/themes/presets'
import type { Mode, Scheme } from '../../lib/themes/roles'

type Props = {
    schemes: Scheme[]
    selectedId: string
    mode: Mode
    deviceIsDark: boolean
    onSelect: (schemeId: string) => void
}

/** Schemes to pick from, each a miniature in its own colours and the variant it would show. */
export function PresetGrid({ schemes, selectedId, mode, deviceIsDark, onSelect }: Props) {
    return (
        <div className="grid gap-8" style={{ gridTemplateColumns: 'repeat(auto-fill, minmax(112px, 1fr))' }}>
            {schemes.map((scheme) => {
                const { roles } = scheme[appearanceOf(scheme, mode, deviceIsDark)]!
                return (
                    <ThemeTile
                        key={scheme.id}
                        name={scheme.name}
                        detail={schemeDetail(scheme)}
                        selected={scheme.id === selectedId}
                        onClick={() => onSelect(scheme.id)}
                        colors={{
                            frame: roles['bg/shell'],
                            content: roles['bg/panel'],
                            line: roles['line/border'],
                            code: [roles['code/keyword'], roles['code/string'], roles['code/function'], roles['code/number']],
                        }}
                    />
                )
            })}
        </div>
    )
}
