import BoringAvatar from "boring-avatars"
import { base_url, api_protocol } from '../services/vizier'

type AvatarVariant = 'beam' | 'marble' | 'pixel' | 'ring' | 'beam_emerald'

const colorPalettes = {
  beam: ["#10B981", "#14B8A6", "#059669", "#047857", "#10B981"],
  marble: ["#6366f1", "#8b5cf6", "#a855f7", "#d946ef", "#ec4899"],
  pixel: ["#f59e0b", "#ef4444", "#ec4899", "#8b5cf6", "#6366f1"],
  ring: ["#3b82f6", "#06b6d4", "#14b8a6", "#10b981", "#84cc16"],
  beam_emerald: ["#10B981", "#14B8A6", "#059669", "#047857", "#10B981"],
}

function resolveUrl(url: string): string {
  if (url.startsWith('http://') || url.startsWith('https://') || url.startsWith('blob:')) return url
  return `${api_protocol}://${base_url}${url}`
}

interface AvatarProps {
  name: string
  rounded?: boolean
  variant?: AvatarVariant
  size?: 'sm' | 'md' | 'lg'
  showStatus?: boolean
  online?: boolean
  avatarUrl?: string
}

const sizeMap = {
  sm: 36,
  md: 48,
  lg: 64,
}

const dotSizeMap = {
  sm: 12,
  md: 16,
  lg: 18,
}

const Avatar = ({
  name,
  rounded = false,
  variant = 'beam',
  size = 'md',
  showStatus = false,
  online = false,
  avatarUrl,
}: AvatarProps) => {
  const colors = colorPalettes[variant]
  const avatarSize = sizeMap[size]
  const dotSize = dotSizeMap[size]

  return (
    <span className="avatar-wrapper">
      {avatarUrl ? (
        <img
          src={resolveUrl(avatarUrl)}
          alt={name}
          style={{
            width: avatarSize,
            height: avatarSize,
            objectFit: 'cover',
            borderRadius: rounded ? '50%' : '8px',
          }}
        />
      ) : (
        <BoringAvatar
          className={`${rounded ? 'rounded-full' : 'rounded-xl'}`}
          name={name}
          variant={variant === 'beam_emerald' ? 'beam' : variant}
          colors={colors}
          square
          size={avatarSize}
        />
      )}
      {showStatus && (
        <span
          className={`avatar-status-dot ${online ? 'online' : ''}`}
          style={{
            width: dotSize,
            height: dotSize,
          }}
        />
      )}
    </span>
  )
}

export default Avatar
export { colorPalettes, type AvatarVariant, type AvatarProps }
