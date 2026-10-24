import { HighlightMatch, Icon, Picker, TextField } from '@vizier/design'
import { useState } from 'react'
import { defaultVoiceOf, voiceSections, voicesOf, type VoiceRow } from '../../lib/mediaModels'
import type { MediaModelsState } from './useMediaModels'

type Props = {
    /** The stored model id; empty means the documented default model. */
    model: string
    /** The stored voice; empty means the documented/host default. */
    voice: string
    /** The local voice map's value, used before the list has loaded. */
    fallbackDefault: string
    media: MediaModelsState
    onChange: (voice: string) => void
}

const trigger =
    'flex h-40 w-full items-center gap-8 rounded-md border border-border-strong bg-ground pr-10 pl-12 text-left type-body text-fg hover:border-border-hover focus-visible:border-live focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-40'

/**
 * The voices the selected speech model publishes (spec 028, FR-022), with a typed fallback. A
 * model that publishes none keeps the free-text field with the documented voice as placeholder,
 * exactly as before this feature.
 */
export function MediaVoiceSelect({ model, voice, fallbackDefault, media, onChange }: Props) {
    const [query, setQuery] = useState('')
    const voices = voicesOf(media.data?.models, model)
    const fallback = defaultVoiceOf(media.data, fallbackDefault)
    const shownVoice = voice || fallback
    const sections = voiceSections(voices, query, fallback)

    if (media.data && voices.length === 0) {
        return (
            <TextField
                label="Voice"
                mono
                value={voice}
                placeholder={fallback || 'Provider default'}
                helper="This model publishes no voice list; type one, or leave it empty for the default."
                onChange={(e) => onChange(e.target.value)}
            />
        )
    }

    return (
        <div className="flex flex-col gap-6">
            <span className="type-ui-medium text-soft">Voice</span>
            <Picker<VoiceRow>
                label="Voice"
                width={360}
                itemHeight={44}
                onOpenChange={(open) => {
                    if (open) media.ensure()
                }}
                trigger={
                    <button type="button" aria-label={`Voice: ${voice || shownVoice || 'none chosen'}`} className={trigger}>
                        <span className={shownVoice ? 'min-w-0 flex-1 truncate' : 'min-w-0 flex-1 truncate text-faint'}>
                            {shownVoice || 'Choose a voice'}
                        </span>
                        <Icon name="chevron-down" size={14} tone="muted" />
                    </button>
                }
                search={{ placeholder: 'Search voices, or type one', total: voices.length ? `${voices.length}` : undefined, value: query, onChange: setQuery }}
                sections={sections}
                getKey={(v) => v.id}
                loading={media.loading && !media.data}
                renderItem={(v, { query: q }) =>
                    v.typed ? (
                        <span className="truncate type-ui text-fg">
                            Use <span className="type-code">{v.id}</span> <span className="text-faint">· not in the list</span>
                        </span>
                    ) : (
                        <span className="flex min-w-0 flex-1 items-center gap-8">
                            <span className="min-w-0 flex-1 truncate type-mono-xs text-fg">
                                <HighlightMatch text={v.id} query={q} />
                            </span>
                            {v.id === fallback && <span className="type-mono-xs text-faint">default</span>}
                        </span>
                    )
                }
                onSelect={(v) => {
                    setQuery('')
                    onChange(v.id)
                }}
                remainder={media.data && voices.length === 0 ? 'This model publishes no voices. Type one above.' : undefined}
            />
            {media.error && <p className="type-small text-failed-text">{media.error}</p>}
        </div>
    )
}
