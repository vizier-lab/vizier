import { useState } from 'react'
import { api_protocol, base_url } from '../services/vizier'
import { useThemeStore } from '../hooks/themeStore'

interface ProviderLogoProps {
  /** Provider or lab id; the logo is served at /api/v1/catalogue/logos/{id}.svg. */
  id: string
  name?: string
  size?: number
  /** The logo has its own colours: don't invert it on a dark theme. */
  fullColor?: boolean
  /** The URL the catalogue gave, when it differs from the id's (a model's lab logo). */
  src?: string | null
}

function initials(name: string): string {
  const words = name.replace(/[^A-Za-z0-9 ]/g, ' ').trim().split(/\s+/)
  const letters = words.length > 1 ? words[0][0] + words[1][0] : name.slice(0, 2)
  return letters.toUpperCase()
}

/**
 * A provider or lab logo at a fixed size, with an initials badge of the same size when there
 * is no logo. Most catalogue logos are drawn in `currentColor`, which an `<img>` renders
 * black, so they are inverted on a dark theme.
 */
export default function ProviderLogo({ id, name, size = 20, fullColor = false, src }: ProviderLogoProps) {
  const [failed, setFailed] = useState(false)
  const dark = useThemeStore((s) => s.resolvedTheme === 'dark')
  const path = src === undefined ? `/api/v1/catalogue/logos/${id}.svg` : src
  const box: React.CSSProperties = {
    width: size,
    height: size,
    flexShrink: 0,
    borderRadius: 4,
  }
  if (!path || failed) {
    return (
      <span
        aria-hidden
        style={{
          ...box,
          display: 'inline-flex',
          alignItems: 'center',
          justifyContent: 'center',
          fontSize: Math.max(8, size * 0.42),
          fontWeight: 600,
          background: 'var(--surface-hover, var(--border))',
          color: 'var(--text-secondary)',
        }}
      >
        {initials(name || id)}
      </span>
    )
  }
  return (
    <img
      src={`${api_protocol}://${base_url}${path}`}
      alt={name || id}
      width={size}
      height={size}
      loading="lazy"
      onError={() => setFailed(true)}
      style={{
        ...box,
        objectFit: 'contain',
        filter: dark && !fullColor ? 'invert(1)' : undefined,
      }}
    />
  )
}
