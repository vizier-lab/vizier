import BoringAvatar from "boring-avatars"

type AvatarVariant = 'beam' | 'marble' | 'pixel' | 'ring' | 'beam_emerald'

const colorPalettes = {
  beam: ["#10B981", "#14B8A6", "#059669", "#047857", "#10B981"],
  marble: ["#6366f1", "#8b5cf6", "#a855f7", "#d946ef", "#ec4899"],
  pixel: ["#f59e0b", "#ef4444", "#ec4899", "#8b5cf6", "#6366f1"],
  ring: ["#3b82f6", "#06b6d4", "#14b8a6", "#10b981", "#84cc16"],
  beam_emerald: ["#10B981", "#14B8A6", "#059669", "#047857", "#10B981"],
}

interface AvatarProps {
  name: string
  rounded?: boolean
  variant?: AvatarVariant
  size?: 'sm' | 'md' | 'lg'
}

const sizeMap = {
  sm: 32,
  md: 48,
  lg: 64,
}

const Avatar = ({ 
  name, 
  rounded = false, 
  variant = 'beam',
  size = 'md'
}: AvatarProps) => {
  const colors = colorPalettes[variant]
  const avatarSize = sizeMap[size]
  
  return (
    <BoringAvatar 
      className={`${rounded ? 'rounded-full' : 'rounded-xl'}`} 
      name={name} 
      variant={variant === 'beam_emerald' ? 'beam' : variant}
      colors={colors} 
      square
      size={avatarSize}
    />
  )
}

export default Avatar
export { colorPalettes, type AvatarVariant, type AvatarProps }
