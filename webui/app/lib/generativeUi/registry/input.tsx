import { Checkbox, Radio, Select, Slider, Switch, TextField, Textarea } from '@vizier/design'
import { useFieldValidation, type ComponentRenderProps } from '@json-render/react'
import { useEmitter, useField } from './shared'

type P = Record<string, unknown>
type Check = { type: string; message: string; args?: Record<string, unknown> }

/** The error messages to show under a field, from its `checks`; validated on blur and by `validateForm`. */
function useChecks(path: string | undefined, checks: unknown) {
    const list = (Array.isArray(checks) ? checks : []) as Check[]
    const validation = useFieldValidation(path ?? '/__unbound', { checks: list as never, validateOn: 'blur' })
    const shown = path && validation.state.touched ? validation.errors : []
    return { error: shown[0], blur: () => path && (validation.touch(), validation.validate()) }
}

const optionsOf = (value: unknown): { value: string; label: string }[] =>
    (Array.isArray(value) ? value : []).map((o) => (typeof o === 'string' ? { value: o, label: o } : { value: String(o?.value ?? ''), label: String(o?.label ?? o?.value ?? '') }))

export function TextInputC({ element, bindings, emit }: ComponentRenderProps<P>) {
    const p = element.props
    const [value, setValue] = useField<string>(p.value as string | undefined, bindings?.value, '')
    const send = useEmitter(emit)
    const { error, blur } = useChecks(bindings?.value, p.checks)
    const common = {
        label: String(p.label ?? ''),
        value,
        disabled: p.disabled === true,
        placeholder: typeof p.placeholder === 'string' ? p.placeholder : undefined,
        maxLength: typeof p.maxLength === 'number' ? p.maxLength : undefined,
        invalid: Boolean(error),
        helper: error,
        onBlur: blur,
    }
    if (p.multiline === true)
        return (
            <Textarea
                {...common}
                onChange={(e) => {
                    setValue(e.target.value)
                    send('change', e.nativeEvent)
                }}
            />
        )
    return (
        <TextField
            {...common}
            onChange={(e) => {
                setValue(e.target.value)
                send('change', e.nativeEvent)
            }}
            onKeyDown={(e) => {
                if (e.key === 'Enter') send('submit', e.nativeEvent)
            }}
        />
    )
}

export function NumberInputC({ element, bindings, emit }: ComponentRenderProps<P>) {
    const p = element.props
    const [value, setValue] = useField<number | ''>(p.value as number | undefined, bindings?.value, '')
    const send = useEmitter(emit)
    const { error, blur } = useChecks(bindings?.value, p.checks)
    return (
        <TextField
            type="number"
            label={String(p.label ?? '')}
            value={value}
            disabled={p.disabled === true}
            min={typeof p.min === 'number' ? p.min : undefined}
            max={typeof p.max === 'number' ? p.max : undefined}
            step={typeof p.step === 'number' ? p.step : undefined}
            invalid={Boolean(error)}
            helper={error}
            onBlur={blur}
            onChange={(e) => {
                setValue(e.target.value === '' ? '' : Number(e.target.value))
                send('change', e.nativeEvent)
            }}
        />
    )
}

export function SliderC({ element, bindings, emit }: ComponentRenderProps<P>) {
    const p = element.props
    const min = typeof p.min === 'number' ? p.min : 0
    const [value, setValue] = useField<number>(p.value as number | undefined, bindings?.value, min)
    const send = useEmitter(emit)
    return (
        <Slider
            label={String(p.label ?? '')}
            value={typeof value === 'number' ? value : min}
            min={min}
            max={typeof p.max === 'number' ? p.max : 100}
            step={typeof p.step === 'number' ? p.step : 1}
            disabled={p.disabled === true}
            onChange={(next) => {
                setValue(next)
                // A drag is a person's gesture, but there is no DOM event to read it from here.
                send('change', { isTrusted: true })
            }}
        />
    )
}

export function SelectC({ element, bindings, emit }: ComponentRenderProps<P>) {
    const p = element.props
    const options = optionsOf(p.options)
    const [value, setValue] = useField<string>(p.value as string | undefined, bindings?.value, '')
    const send = useEmitter(emit)
    const { error, blur } = useChecks(bindings?.value, p.checks)
    return (
        <Select
            label={String(p.label ?? '')}
            options={options}
            value={value || undefined}
            disabled={p.disabled === true}
            invalid={Boolean(error)}
            helper={error}
            onChange={(next) => {
                setValue(next)
                blur()
                send('change', { isTrusted: true })
            }}
        />
    )
}

export function CheckboxC({ element, bindings, emit }: ComponentRenderProps<P>) {
    const p = element.props
    const [value, setValue] = useField<boolean>(p.value as boolean | undefined, bindings?.value, false)
    const send = useEmitter(emit)
    return (
        <Checkbox
            label={String(p.label ?? '')}
            checked={value === true}
            disabled={p.disabled === true}
            onChange={(e) => {
                setValue(e.target.checked)
                send('change', e.nativeEvent)
            }}
        />
    )
}

export function SwitchC({ element, bindings, emit }: ComponentRenderProps<P>) {
    const p = element.props
    const [value, setValue] = useField<boolean>(p.value as boolean | undefined, bindings?.value, false)
    const send = useEmitter(emit)
    return (
        <label className="inline-flex items-center gap-8 type-ui text-fg">
            <Switch
                checked={value === true}
                disabled={p.disabled === true}
                aria-label={String(p.label ?? '')}
                onCheckedChange={(next) => {
                    setValue(next)
                    send('change', { isTrusted: true })
                }}
            />
            {String(p.label ?? '')}
        </label>
    )
}

export function RadioGroupC({ element, bindings, emit }: ComponentRenderProps<P>) {
    const p = element.props
    const options = optionsOf(p.options)
    const [value, setValue] = useField<string>(p.value as string | undefined, bindings?.value, '')
    const send = useEmitter(emit)
    const name = `radio-${element.type}-${String(p.label ?? '')}`
    return (
        <fieldset className="flex min-w-0 flex-col gap-8" disabled={p.disabled === true}>
            <legend className="type-ui-medium text-soft">{String(p.label ?? '')}</legend>
            {options.map((o) => (
                <Radio
                    key={o.value}
                    name={name}
                    label={o.label}
                    checked={value === o.value}
                    onChange={(e) => {
                        setValue(o.value)
                        send('change', e.nativeEvent)
                    }}
                />
            ))}
        </fieldset>
    )
}
