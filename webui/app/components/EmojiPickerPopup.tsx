import { useState, useRef, useEffect, memo } from 'react'

const EMOJI_CATEGORIES: { name: string; icon: string; emojis: string[] }[] = [
  {
    name: 'Smileys',
    icon: '😀',
    emojis: [
      '😀', '😃', '😄', '😁', '😆', '😅', '🤣', '😂', '🙂', '😉',
      '😊', '😇', '🥰', '😍', '🤩', '😘', '😗', '😚', '😙', '🥲',
      '😋', '😛', '😜', '🤪', '😝', '🤑', '🤗', '🤭', '🤫', '🤔',
      '🫡', '🤐', '🤨', '😐', '😑', '😶', '😏', '😒', '🙄', '😬',
      '😮‍💨', '🤥', '😌', '😔', '😪', '🤤', '😴', '😷', '🤒', '🤕',
      '🤢', '🤮', '🥵', '🥶', '🥴', '😵', '🤯', '🤠', '🥳', '🥸',
      '😎', '🤓', '🧐', '😕', '🫤', '😟', '🙁', '😮', '😯', '😲',
      '😳', '🥺', '🥹', '😦', '😧', '😨', '😰', '😥', '😢', '😭',
      '😱', '😖', '😣', '😞', '😓', '😩', '😫', '🥱', '😤', '😡',
      '😠', '🤬', '😈', '👿', '💀', '☠️', '💩', '🤡', '👹', '👺',
      '👻', '👽', '👾', '🤖', '😺', '😸', '😹', '😻', '😼', '😽',
    ],
  },
  {
    name: 'Animals',
    icon: '🐶',
    emojis: [
      '🐶', '🐱', '🐭', '🐹', '🐰', '🦊', '🐻', '🐼', '🐻‍❄️', '🐨',
      '🐯', '🦁', '🐮', '🐷', '🐸', '🐵', '🙈', '🙉', '🙊', '🐒',
      '🐔', '🐧', '🐦', '🐤', '🐣', '🐥', '🦆', '🦅', '🦉', '🦇',
      '🐺', '🐗', '🐴', '🦄', '🐝', '🪱', '🐛', '🦋', '🐌', '🐞',
      '🐜', '🪰', '🪲', '🪳', '🦟', '🦗', '🕷', '🦂', '🐢', '🐍',
      '🦎', '🦖', '🦕', '🐙', '🦑', '🦐', '🦞', '🦀', '🐡', '🐠',
      '🐟', '🐬', '🐳', '🐋', '🦈', '🐊', '🐅', '🐆', '🦓', '🦍',
      '🦧', '🐘', '🦛', '🦏', '🐪', '🐫', '🦒', '🦘', '🦬', '🐃',
    ],
  },
  {
    name: 'Food',
    icon: '🍎',
    emojis: [
      '🍎', '🍐', '🍊', '🍋', '🍌', '🍉', '🍇', '🍓', '🍈', '🍒',
      '🍑', '🥭', '🍍', '🥥', '🥝', '🍅', '🍆', '🥑', '🫑', '🥦',
      '🥬', '🥒', '🌶', '🫑', '🌽', '🥕', '🫒', '🧄', '🧅', '🥔',
      '🍠', '🥐', '🥯', '🍞', '🥖', '🥨', '🧀', '🥚', '🍳', '🧈',
      '🥞', '🧇', '🥓', '🥩', '🍗', '🍖', '🌭', '🍔', '🍟', '🍕',
      '🫓', '🥪', '🥙', '🧆', '🌮', '🌯', '🫔', '🥗', '🥘', '🫕',
      '🥫', '🍝', '🍜', '🍲', '🍛', '🍣', '🍱', '🥟', '🦪', '🍤',
      '🍙', '🍚', '🍘', '🍥', '🥠', '🥮', '🍢', '🍡', '🍧', '🍨',
    ],
  },
  {
    name: 'Activities',
    icon: '⚽',
    emojis: [
      '⚽', '🏀', '🏈', '⚾', '🥎', '🎾', '🏐', '🏉', '🥏', '🎱',
      '🪀', '🏓', '🏸', '🏒', '🏑', '🥍', '🏏', '🪃', '🥅', '⛳',
      '🪁', '🏹', '🎣', '🤿', '🥊', '🥋', '🎽', '🛹', '🛼', '🛷',
      '⛸', '🥌', '🎿', '🛷', '🎯', '🪀', '🎮', '🕹', '🎲', '🧩',
      '🎭', '🎨', '🧵', '🧶', '🎪', '🎤', '🎧', '🎼', '🎹', '🥁',
      '🪘', '🎷', '🎺', '🪗', '🎸', '🪕', '🎻', '🎲', '♟', '🎯',
      '🎳', '🎮', '🎰', '🎪', '🎫', '🎟', '🎗', '🏅', '🥇', '🥈',
      '🥉', '🏆', '🏵', '🎗', '🎖', '🏆', '🥇', '🥈', '🥉', '🏅',
    ],
  },
  {
    name: 'Travel',
    icon: '🚗',
    emojis: [
      '🚗', '🚕', '🚙', '🚌', '🚎', '🏎', '🚓', '🚑', '🚒', '🚐',
      '🛻', '🚚', '🚛', '🚜', '🏍', '🛵', '🚲', '🛴', '🛹', '🛼',
      '🚁', '✈', '🛩', '🚀', '🛸', '🚢', '⛵', '🛶', '🚤', '🛥',
      '🗺', '🧭', '⛰', '🏔', '🌋', '🗻', '🏕', '🏖', '🏜', '🏝',
      '🏞', '🏟', '🏛', '🏗', '🧱', '🪨', '🪵', '🛖', '🏘', '🏚',
      '🏠', '🏡', '🏢', '🏣', '🏤', '🏥', '🏦', '🏨', '🏩', '🏪',
      '🏫', '🏬', '🏭', '🏯', '🏰', '💒', '🗼', '🗽', '⛪', '🕌',
      '🛕', '🕍', '⛩', '🕋', '⛲', '⛺', '🌁', '🌃', '🏙', '🌄',
    ],
  },
  {
    name: 'Objects',
    icon: '💡',
    emojis: [
      '⌚', '📱', '📲', '💻', '⌨', '🖥', '🖨', '🖱', '🖲', '🕹',
      '🗜', '💽', '💾', '💿', '📀', '📼', '📷', '📸', '📹', '🎥',
      '📽', '🎞', '📞', '☎', '📟', '📠', '📺', '📻', '🎙', '🎚',
      '🎛', '🧭', '⏱', '⏲', '⏰', '🕰', '📡', '🔋', '🔌', '💡',
      '🔦', '🕯', '🪔', '🧯', '🛢', '💸', '💵', '💴', '💶', '💷',
      '🪙', '💰', '💳', '⚖', '🪜', '🧰', '🪛', '🔧', '🔩', '⚙',
      '🗜', '⛏', '🛠', '⚒', '🔨', '🪓', '🗡', '⚔', '🔫', '🪃',
      '🏹', '🛡', '🪚', '🔧', '🪛', '🔩', '⚙', '🗜', '⚖', '🦯',
    ],
  },
  {
    name: 'Symbols',
    icon: '❤️',
    emojis: [
      '❤️', '🧡', '💛', '💚', '💙', '💜', '🖤', '🤍', '🤎', '💔',
      '❣️', '💕', '💞', '💓', '💗', '💖', '💘', '💝', '💟', '☮',
      '✝', '☪', '🕉', '☸', '✡', '🔯', '🕎', '☯', '☦', '🛐',
      '⛎', '♈', '♉', '♊', '♋', '♌', '♍', '♎', '♏', '♐',
      '♑', '♒', '♓', '🆔', '⚛', '🉑', '☢', '☣', '📴', '📳',
      '🈶', '🈚', '🈸', '🈺', '🈷', '✴', '🆚', '💮', '🉐', '㊙',
      '㊗', '🈴', '🈵', '🈹', '🈲', '🅰', '🅱', '🆎', '🆑', '🅾',
      '🆘', '❌', '⭕', '🛑', '⛔', '📛', '🚫', '💯', '💢', '♨',
    ],
  },
  {
    name: 'Flags',
    icon: '🏁',
    emojis: [
      '🏁', '🚩', '🎌', '🏴', '🏳', '🏳️‍🌈', '🏳️‍⚧️', '🏴‍☠️', '🇺🇳', '🇦🇫',
      '🇦🇱', '🇩🇿', '🇦🇸', '🇦🇩', '🇦🇴', '🇦🇮', '🇦🇶', '🇦🇬', '🇦🇷', '🇦🇲',
      '🇦🇼', '🇦🇺', '🇦🇹', '🇦🇿', '🇧🇸', '🇧🇭', '🇧🇩', '🇧🇧', '🇧🇾', '🇧🇪',
      '🇧🇿', '🇧🇯', '🇧🇲', '🇧🇹', '🇧🇴', '🇧🇦', '🇧🇼', '🇧🇷', '🇮🇴', '🇻🇬',
      '🇧🇳', '🇧🇬', '🇧🇫', '🇧🇮', '🇰🇭', '🇨🇲', '🇨🇦', '🇨🇻', '🇰🇾', '🇨🇫',
      '🇹🇩', '🇨🇱', '🇨🇳', '🇨🇴', '🇰🇲', '🇨🇬', '🇨🇩', '🇨🇰', '🇨🇷', '🇨🇮',
      '🇭🇷', '🇨🇺', '🇨🇼', '🇨🇾', '🇨🇿', '🇩🇰', '🇩🇯', '🇩🇲', '🇩🇴', '🇪🇨',
    ],
  },
]

interface EmojiPickerPopupProps {
  onSelect: (emoji: string) => void
  onClose: () => void
}

function EmojiPickerPopupComponent({ onSelect, onClose }: EmojiPickerPopupProps) {
  const [search, setSearch] = useState('')
  const [activeCategory, setActiveCategory] = useState(0)
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        onClose()
      }
    }
    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [onClose])

  const displayEmojis = search
    ? EMOJI_CATEGORIES.flatMap((cat) => cat.emojis).filter(() => true)
    : EMOJI_CATEGORIES[activeCategory].emojis

  return (
    <div
      ref={ref}
      style={{
        position: 'absolute',
        bottom: '100%',
        left: '0',
        marginBottom: '4px',
        background: 'var(--surface)',
        border: '1px solid var(--border)',
        borderRadius: '8px',
        padding: '8px',
        boxShadow: 'var(--shadow-md)',
        zIndex: 100,
        width: '320px',
      }}
    >
      <input
        type="text"
        placeholder="Search emoji..."
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        style={{
          width: '100%',
          padding: '6px 8px',
          border: '1px solid var(--border)',
          borderRadius: '4px',
          background: 'var(--background)',
          color: 'var(--text-primary)',
          fontSize: '13px',
          marginBottom: '8px',
          outline: 'none',
          boxSizing: 'border-box',
        }}
      />
      <div
        style={{
          display: 'flex',
          gap: '2px',
          marginBottom: '8px',
          borderBottom: '1px solid var(--border)',
          paddingBottom: '6px',
          overflowX: 'auto',
        }}
      >
        {EMOJI_CATEGORIES.map((cat, idx) => (
          <button
            key={cat.name}
            onClick={() => {
              setActiveCategory(idx)
              setSearch('')
            }}
            title={cat.name}
            style={{
              background: activeCategory === idx && !search ? 'var(--border)' : 'none',
              border: 'none',
              cursor: 'pointer',
              padding: '4px 6px',
              borderRadius: '4px',
              fontSize: '16px',
              lineHeight: 1,
              transition: 'background 0.15s',
              flexShrink: 0,
            }}
            onMouseEnter={(e) => {
              if (activeCategory !== idx || search) {
                (e.target as HTMLElement).style.background = 'var(--border)'
              }
            }}
            onMouseLeave={(e) => {
              if (activeCategory !== idx || search) {
                (e.target as HTMLElement).style.background = 'none'
              }
            }}
          >
            {cat.icon}
          </button>
        ))}
      </div>
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(8, 1fr)',
          gap: '2px',
          maxHeight: '200px',
          overflowY: 'auto',
        }}
      >
        {displayEmojis.map((emoji, idx) => (
          <button
            key={`${emoji}-${idx}`}
            onClick={() => {
              onSelect(emoji)
              onClose()
            }}
            style={{
              background: 'none',
              border: 'none',
              cursor: 'pointer',
              padding: '6px',
              borderRadius: '4px',
              fontSize: '18px',
              lineHeight: 1,
              transition: 'background 0.15s',
            }}
            onMouseEnter={(e) => {
              (e.target as HTMLElement).style.background = 'var(--border)'
            }}
            onMouseLeave={(e) => {
              (e.target as HTMLElement).style.background = 'none'
            }}
          >
            {emoji}
          </button>
        ))}
      </div>
    </div>
  )
}

export const EmojiPickerPopup = memo(EmojiPickerPopupComponent)
