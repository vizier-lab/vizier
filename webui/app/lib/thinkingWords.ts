import { useEffect, useState } from 'react'

const WORDS = [
    'thinking',
    'pondering',
    'reasoning',
    'considering',
    'analyzing',
    'contemplating',
    'deliberating',
    'reflecting',
    'musing',
    'figuring it out',
    'working on it',
    'brewing ideas',
    'cooking up a response',
    'consulting the archives',
    'doing some brain yoga',
    'gathering thoughts',
    'having a think',
    'mapping it out',
    'putting pieces together',
    'reading between the lines',
    'rubbing my chin',
    'stirring the pot',
    'tracing the logic',
    'unpacking this',
    'warming up the brain',
    'weighing options',
    'wrestling with ideas',
    'consulting the grand vizier',
    'deciphering royal scrolls',
    'distilling ancient wisdom',
    'entering the oracle chamber',
    'gazing into the crystal ball',
    'heeding the inner counsel',
    'peering through the astrolabe',
    'seeking the council of elders',
    'sharpening the vizier’s blade',
    'sorting the palace archives',
    'unraveling the mystery',
    'weighing the scrolls',
]

/** One of the idle words a turn shows while it works; never `not`, so a rotation always changes. */
export const thinkingWord = (not?: string) => {
    const pool = WORDS.filter((w) => w !== not)
    return pool[Math.floor(Math.random() * pool.length)]
}

/** How long each thinking line stays before the next. */
export const THINKING_WORD_MS = 10_000

/**
 * The word on a line that says what the agent is doing, while it does it: one at once, and a
 * different one every `ms` for as long as `live`. The activity trail and the `/btw` side chat
 * share it, so both read the same way.
 */
export function useThinkingWord(live: boolean, ms = THINKING_WORD_MS): string {
    const [word, setWord] = useState(() => thinkingWord())
    useEffect(() => {
        if (!live) return
        const timer = setInterval(() => setWord((w) => thinkingWord(w)), ms)
        return () => clearInterval(timer)
    }, [live, ms])
    return word
}
