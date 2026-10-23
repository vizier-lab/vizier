import { useState, useRef, useEffect, useMemo, memo } from 'react'

const EMOJI_CATEGORIES: { name: string; icon: string; emojis: string[] }[] = [
  {
    name: 'Smileys',
    icon: '😀',
    emojis: [
      '😀', '😃', '😄', '😁', '😆', '😅', '🤣', '😂', '🙂', '😉',
      '😊', '😇', '🥰', '😍', '🤩', '😘', '😗', '😚', '😙', '🥲',
      '😋', '😛', '😜', '🤪', '😝', '🤑', '🤗', '🤭', '🤫', '🤔',
      '🤐', '🤨', '😐', '😑', '😶', '😏', '😒', '🙄', '😬',
      '🤥', '😌', '😔', '😪', '🤤', '😴', '😷', '🤒', '🤕',
      '🤢', '🤮', '🥵', '🥶', '🥴', '😵', '🤯', '🤠', '🥳', '🥸',
      '😎', '🤓', '🧐', '😕', '😟', '🙁', '😮', '😯', '😲',
      '😳', '🥺', '🥹', '😦', '😧', '😨', '😰', '😥', '😢', '😭',
      '😱', '😖', '😣', '😞', '😓', '😩', '😫', '🥱', '😤', '😡',
      '😠', '🤬', '😈', '👿', '💀', '☠️', '💩', '🤡', '👹', '👺',
      '👻', '👽', '👾', '🤖',
    ],
  },
  {
    name: 'Hands',
    icon: '👋',
    emojis: [
      '👋', '🤚', '✋', '🖖', '👌', '🤌', '🤏', '✌', '🤞', '🤟',
      '🤘', '🤙', '👈', '👉', '👆', '🖕', '👇', '☝',
      '👍', '👎', '✊', '👊', '🤛', '🤜', '👏', '🙌', '🫶', '👐',
      '🤲', '🤝', '🙏', '💪', '🦾',
    ],
  },
  {
    name: 'Animals',
    icon: '🐶',
    emojis: [
      '🐶', '🐱', '🐭', '🐹', '🐰', '🦊', '🐻', '🐼', '🐨',
      '🐯', '🦁', '🐮', '🐷', '🐸', '🐵', '🙈', '🙉', '🙊', '🐒',
      '🐔', '🐧', '🐦', '🐤', '🐣', '🐥', '🦆', '🦅', '🦉', '🦇',
      '🐺', '🐗', '🐴', '🦄', '🐝', '🐛', '🦋', '🐌', '🐞',
      '🐜', '🕷', '🐢', '🐍', '🦎', '🐙', '🦑', '🦐', '🦞', '🦀',
      '🐡', '🐠', '🐟', '🐬', '🐳', '🐋', '🦈', '🐊', '🐅', '🐆',
    ],
  },
  {
    name: 'Food',
    icon: '🍎',
    emojis: [
      '🍎', '🍐', '🍊', '🍋', '🍌', '🍉', '🍇', '🍓', '🍒',
      '🍑', '🥭', '🍍', '🥥', '🥝', '🍅', '🍆', '🥑', '🥦',
      '🥒', '🌶', '🌽', '🥕', '🍞', '🥖', '🧀', '🥚', '🍳',
      '🥞', '🥓', '🥩', '🍗', '🍖', '🌭', '🍔', '🍟', '🍕',
      '🥪', '🌮', '🌯', '🥗', '🍝', '🍜', '🍲', '🍛', '🍣', '🍱',
      '🥟', '🍙', '🍚', '🍰', '🎂', '🍭', '🍬', '🍫', '🍿', '🍩',
      '🍪', '☕', '🍵', '🍶', '🍾', '🍷', '🍸', '🍹', '🍺', '🍻', '🥂',
    ],
  },
  {
    name: 'Activities',
    icon: '⚽',
    emojis: [
      '⚽', '🏀', '🏈', '⚾', '🎾', '🏐', '🎱', '🏓', '🏸',
      '⛳', '🏹', '🎣', '🥊', '🥋', '🎮', '🎲', '🧩',
      '🎭', '🎨', '🎤', '🎧', '🎹', '🥁', '🎷', '🎺', '🎸',
      '🎻', '🎯', '🎳', '🏆', '🥇', '🥈', '🥉', '🏅',
    ],
  },
  {
    name: 'Travel',
    icon: '🚗',
    emojis: [
      '🚗', '🚕', '🚙', '🚌', '🏎', '🚓', '🚑', '🚒', '🚐',
      '🚚', '🚛', '🚜', '🏍', '🛵', '🚲', '🛴',
      '✈', '🚀', '🛸', '🚢', '⛵', '🚤',
      '⛰', '🏔', '🌋', '🏕', '🏖', '🏝',
      '🏠', '🏡', '🏢', '🏥', '🏦', '🏨', '🏪', '🏫', '🏬',
      '🏯', '🏰', '🗼', '🗽', '⛪', '🕌', '⛩', '⛲', '⛺',
    ],
  },
  {
    name: 'Objects',
    icon: '💡',
    emojis: [
      '⌚', '📱', '💻', '🖥', '📷', '📸', '📹', '🎥',
      '📞', '☎', '📺', '📻', '⏰', '🔔', '🔓',
      '💡', '🔦', '🕯', '💰', '💳', '🔧', '🔩', '⚙', '🔨',
      '🔑', '🚪', '🎁', '🎈', '🎉', '🎊', '🎀', '🎄', '🎃',
      '🎅', '🤶', '🔮', '🔬', '💊', '💉', '🌡', '🧹',
      '🔑', '🗝', '🛠', '🧭', '📡', '🔋', '🔌',
    ],
  },
  {
    name: 'Symbols',
    icon: '❤️',
    emojis: [
      '❤️', '🧡', '💛', '💚', '💙', '💜', '🖤', '🤍', '🤎', '💔',
      '❣️', '💕', '💞', '💓', '💗', '💖', '💘', '💝', '💟',
      '✅', '❌', '⭕', '🛑', '⛔', '💯', '💢', '♨',
      '☮', '✝', '☪', '🕉', '☸', '✡', '☯',
      '♈', '♉', '♊', '♋', '♌', '♍', '♎', '♏', '♐', '♑', '♒', '♓',
    ],
  },
  {
    name: 'Flags',
    icon: '🏁',
    emojis: [
      '🏁', '🚩', '🎌', '🏴', '🏳', '🏳️‍🌈', '🏳️‍⚧️', '🏴‍☠️',
      '🇺🇸', '🇬🇧', '🇯🇵', '🇰🇷', '🇨🇳', '🇩🇪', '🇫🇷', '🇮🇹',
      '🇪🇸', '🇧🇷', '🇮🇳', '🇷🇺', '🇦🇺', '🇨🇦', '🇲🇽',
    ],
  },
]

const EMOJI_KEYWORDS: Record<string, string[]> = {
  '😀': ['happy', 'smile', 'grin'],
  '😃': ['happy', 'smile'],
  '😄': ['happy', 'laugh'],
  '😁': ['grin', 'happy'],
  '😆': ['laugh', 'happy'],
  '😅': ['sweat', 'nervous'],
  '🤣': ['rofl', 'laugh', 'rolling'],
  '😂': ['cry', 'laugh', 'joy', 'tears'],
  '🙂': ['smile', 'slight'],
  '😉': ['wink'],
  '😊': ['blush', 'happy', 'shy'],
  '😇': ['angel', 'innocent'],
  '🥰': ['love', 'hearts'],
  '😍': ['heart', 'eyes', 'love'],
  '🤩': ['star', 'excited', 'wow'],
  '😘': ['kiss', 'love'],
  '😋': ['yum', 'delicious'],
  '😛': ['tongue', 'playful'],
  '🤑': ['money', 'rich'],
  '🤗': ['hug', 'hugs'],
  '🤫': ['shh', 'quiet', 'secret'],
  '🤔': ['think', 'hmm', 'wonder'],
  '😏': ['smirk', 'suggestive'],
  '😒': ['unamused', 'annoyed'],
  '🙄': ['eye', 'roll', 'whatever'],
  '😬': ['grimace', 'awkward'],
  '😌': ['relieved', 'calm'],
  '😔': ['sad', 'pensive'],
  '😴': ['sleep', 'zzz', 'snore'],
  '😷': ['mask', 'sick', 'doctor'],
  '🤒': ['sick', 'fever'],
  '🤢': ['sick', 'nauseous'],
  '🤮': ['vomit', 'sick', 'puke'],
  '🥵': ['hot', 'sweat'],
  '🥶': ['cold', 'freeze', 'ice'],
  '🤯': ['mind', 'blown', 'shock'],
  '🤠': ['cowboy', 'hat'],
  '🥳': ['party', 'celebrate', 'birthday'],
  '😎': ['cool', 'sunglasses'],
  '🤓': ['nerd', 'geek', 'glasses'],
  '😟': ['worried', 'anxious'],
  '😮': ['surprise', 'wow'],
  '😲': ['shocked', 'astonished'],
  '😳': ['embarrassed', 'shy'],
  '🥺': ['pleading', 'beg', 'cute'],
  '😢': ['cry', 'sad', 'tear'],
  '😭': ['sob', 'cry', 'loud', 'sad'],
  '😱': ['scream', 'fear', 'horror'],
  '😞': ['disappointed', 'sad'],
  '😩': ['weary', 'tired'],
  '😤': ['triumph', 'angry', 'steam'],
  '😡': ['angry', 'mad', 'rage'],
  '😠': ['angry', 'mad'],
  '🤬': ['cursing', 'angry'],
  '😈': ['devil', 'evil'],
  '👿': ['devil', 'imp'],
  '💀': ['skull', 'dead', 'death'],
  '💩': ['poop', 'poo'],
  '🤡': ['clown', 'joke'],
  '👻': ['ghost', 'halloween', 'boo'],
  '👽': ['alien', 'ufo'],
  '🤖': ['robot', 'bot'],
  '❤️': ['heart', 'love', 'red'],
  '🧡': ['orange', 'heart'],
  '💛': ['yellow', 'heart'],
  '💚': ['green', 'heart'],
  '💙': ['blue', 'heart'],
  '💜': ['purple', 'heart'],
  '🖤': ['black', 'heart'],
  '🤍': ['white', 'heart'],
  '💔': ['broken', 'heart', 'sad'],
  '💕': ['two', 'hearts', 'love'],
  '💞': ['revolving', 'hearts'],
  '💓': ['beating', 'heart'],
  '💗': ['growing', 'heart'],
  '💖': ['sparkling', 'heart'],
  '💘': ['heart', 'arrow', 'cupid'],
  '💝': ['heart', 'ribbon', 'gift'],
  '💯': ['hundred', 'perfect', '100'],
  '💥': ['collision', 'boom', 'crash'],
  '💫': ['dizzy', 'star', 'sparkle'],
  '💬': ['speech', 'bubble', 'chat'],
  '💭': ['thought', 'bubble', 'think'],
  '💤': ['zzz', 'sleep', 'snore'],
  '👋': ['wave', 'hello', 'hi', 'bye'],
  '✋': ['hand', 'stop', 'high five'],
  '👌': ['ok', 'perfect', 'good'],
  '✌': ['peace', 'victory'],
  '🤞': ['crossed', 'fingers', 'luck'],
  '🤟': ['love', 'you'],
  '🤘': ['rock', 'horns'],
  '🤙': ['call', 'me', 'shaka'],
  '👈': ['point', 'left'],
  '👉': ['point', 'right'],
  '👆': ['point', 'up'],
  '👇': ['point', 'down'],
  '👍': ['thumbs', 'up', 'like', 'yes', 'good', 'ok', 'agree'],
  '👎': ['thumbs', 'down', 'no', 'bad', 'dislike'],
  '✊': ['fist', 'power', 'solidarity'],
  '👊': ['fist', 'punch', 'bump'],
  '👏': ['clap', 'applause', 'bravo', 'congrats'],
  '🙌': ['raised', 'hands', 'celebrate', 'hooray'],
  '🤝': ['handshake', 'deal', 'agree'],
  '🙏': ['pray', 'please', 'thank', 'namaste'],
  '💪': ['muscle', 'strong', 'flex', 'gym'],
  '👀': ['eyes', 'look', 'see', 'watch'],
  '👅': ['tongue', 'taste'],
  '👄': ['mouth', 'lips'],
  '💋': ['kiss', 'lips'],
  '🐶': ['dog', 'puppy', 'pet'],
  '🐱': ['cat', 'kitten', 'pet'],
  '🐭': ['mouse'],
  '🐰': ['rabbit', 'bunny'],
  '🦊': ['fox', 'clever'],
  '🐻': ['bear', 'teddy'],
  '🐼': ['panda'],
  '🐨': ['koala'],
  '🐯': ['tiger'],
  '🦁': ['lion', 'king'],
  '🐮': ['cow', 'moo'],
  '🐷': ['pig', 'oink'],
  '🐸': ['frog', 'toad'],
  '🐵': ['monkey', 'ape'],
  '🦄': ['unicorn', 'magic'],
  '🐝': ['bee', 'honey', 'buzz'],
  '🦋': ['butterfly'],
  '🐢': ['turtle', 'shell'],
  '🐍': ['snake'],
  '🐙': ['octopus'],
  '🐟': ['fish'],
  '🐬': ['dolphin'],
  '🐳': ['whale'],
  '🦈': ['shark'],
  '🍎': ['apple', 'red', 'fruit'],
  '🍊': ['orange', 'fruit', 'citrus'],
  '🍋': ['lemon', 'sour'],
  '🍌': ['banana'],
  '🍉': ['watermelon'],
  '🍇': ['grape', 'grapes'],
  '🍓': ['strawberry'],
  '🍒': ['cherry'],
  '🍑': ['peach'],
  '🍅': ['tomato'],
  '🍆': ['eggplant'],
  '🥑': ['avocado'],
  '🍔': ['burger', 'hamburger'],
  '🍟': ['fries', 'chips'],
  '🍕': ['pizza'],
  '🌮': ['taco'],
  '🍣': ['sushi'],
  '☕': ['coffee', 'hot', 'tea'],
  '🍺': ['beer', 'mug'],
  '🍻': ['beers', 'cheers'],
  '🥂': ['champagne', 'cheers', 'celebrate'],
  '🍷': ['wine'],
  '🍸': ['cocktail', 'martini'],
  '🍹': ['tropical', 'drink'],
  '⚽': ['soccer', 'ball', 'football'],
  '🏀': ['basketball', 'ball'],
  '🏈': ['football', 'american'],
  '⚾': ['baseball'],
  '🎾': ['tennis'],
  '🎮': ['video', 'game', 'controller'],
  '🎲': ['dice', 'game'],
  '🧩': ['puzzle', 'jigsaw'],
  '🎯': ['target', 'bullseye', 'dart'],
  '🏆': ['trophy', 'winner', 'champion'],
  '🥇': ['gold', 'medal', 'first'],
  '🥈': ['silver', 'medal'],
  '🥉': ['bronze', 'medal'],
  '🚗': ['car', 'drive'],
  '✈': ['airplane', 'plane', 'fly'],
  '🚀': ['rocket', 'space'],
  '🚢': ['ship', 'boat'],
  '⛰': ['mountain'],
  '🏖': ['beach', 'sand'],
  '🏠': ['house', 'home'],
  '🏰': ['castle', 'palace'],
  '🗼': ['tower'],
  '🗽': ['statue', 'liberty'],
  '⛪': ['church'],
  '📱': ['phone', 'mobile', 'cell'],
  '💻': ['computer', 'laptop'],
  '📷': ['camera', 'photo'],
  '⏰': ['alarm', 'clock', 'time'],
  '💡': ['light', 'bulb', 'idea'],
  '🔑': ['key', 'lock'],
  '🎁': ['gift', 'present', 'birthday'],
  '🎈': ['balloon', 'party'],
  '🎉': ['party', 'celebrate', 'tada', 'birthday'],
  '🎊': ['confetti', 'party'],
  '🎄': ['christmas', 'tree'],
  '🎃': ['halloween', 'pumpkin'],
  '🎅': ['santa', 'christmas'],
  '✅': ['check', 'yes', 'done', 'correct'],
  '❌': ['cross', 'no', 'wrong'],
  '⭕': ['circle', 'o'],
  '🛑': ['stop', 'sign'],
  '⛔': ['no', 'entry', 'forbidden'],
  '🔴': ['red', 'circle'],
  '🟠': ['orange', 'circle'],
  '🟡': ['yellow', 'circle'],
  '🟢': ['green', 'circle'],
  '🔵': ['blue', 'circle'],
  '🟣': ['purple', 'circle'],
  '⚫': ['black', 'circle'],
  '⚪': ['white', 'circle'],
  '🏁': ['checkered', 'flag', 'race'],
  '🚩': ['red', 'flag'],
  '🏳️‍🌈': ['rainbow', 'flag', 'pride'],
  '🇺🇸': ['usa', 'america'],
  '🇬🇧': ['uk', 'britain', 'england'],
  '🇯🇵': ['japan'],
  '🇰🇷': ['korea'],
  '🇨🇳': ['china'],
  '🇩🇪': ['germany'],
  '🇫🇷': ['france'],
  '🇮🇹': ['italy'],
  '🇪🇸': ['spain'],
  '🇧🇷': ['brazil'],
  '🇮🇳': ['india'],
}

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

  const displayEmojis = useMemo(() => {
    if (!search) {
      return EMOJI_CATEGORIES[activeCategory].emojis
    }
    const lower = search.toLowerCase()
    return EMOJI_CATEGORIES.flatMap((cat) => cat.emojis).filter((emoji) => {
      const keywords = EMOJI_KEYWORDS[emoji]
      return keywords?.some((k) => k.toLowerCase().includes(lower)) ?? false
    })
  }, [search, activeCategory])

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
      {!search && (
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
              onClick={() => setActiveCategory(idx)}
              title={cat.name}
              style={{
                background: activeCategory === idx ? 'var(--border)' : 'none',
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
                if (activeCategory !== idx) {
                  (e.target as HTMLElement).style.background = 'var(--border)'
                }
              }}
              onMouseLeave={(e) => {
                if (activeCategory !== idx) {
                  (e.target as HTMLElement).style.background = 'none'
                }
              }}
            >
              {cat.icon}
            </button>
          ))}
        </div>
      )}
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
