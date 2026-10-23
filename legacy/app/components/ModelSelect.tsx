import { useState, useEffect, useRef } from 'react'

interface ModelSelectProps {
  options: string[]
  value: string
  onChange: (value: string) => void
  placeholder?: string
  style?: React.CSSProperties
}

const CUSTOM_VALUE = '__custom__'

export default function ModelSelect({ options, value, onChange, placeholder, style }: ModelSelectProps) {
  const [isCustom, setIsCustom] = useState(false)
  const userToggledRef = useRef(false)

  useEffect(() => {
    if (userToggledRef.current) {
      userToggledRef.current = false
      return
    }
    if (value && !options.includes(value)) {
      setIsCustom(true)
    } else if (!value || options.includes(value)) {
      setIsCustom(false)
    }
  }, [value, options])

  const handleSelectChange = (selected: string) => {
    if (selected === CUSTOM_VALUE) {
      userToggledRef.current = true
      setIsCustom(true)
      onChange('')
    } else {
      setIsCustom(false)
      onChange(selected)
    }
  }

  const currentValue = isCustom ? CUSTOM_VALUE : (value || '')

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
      <select
        style={style}
        value={currentValue}
        onChange={(e) => handleSelectChange(e.target.value)}
      >
        {!value && !isCustom && (
          <option value="" disabled>{placeholder || 'Select...'}</option>
        )}
        {options.map((opt) => (
          <option key={opt} value={opt}>{opt}</option>
        ))}
        <option value={CUSTOM_VALUE}>Custom...</option>
      </select>
      {isCustom && (
        <input
          style={style}
          type="text"
          value={value || ''}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder || 'Enter custom value...'}
          autoFocus
        />
      )}
    </div>
  )
}
