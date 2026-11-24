/**
 * The notification sound: played when a toast appears and when an agent's reply arrives on an
 * open chat. One short clip (`public/notify-signature.*`), so a burst of events is one sound.
 */

/** Events closer than this are one sound: a reply and the toast it raises, a burst of toasts. */
export const MIN_GAP_MS = 400

/** The file is mastered loud (-1 dBFS peak); the page plays it at this level. */
export const VOLUME = 0.12

/** Whether a sound requested at `now` should play, given when the last one did. */
export function shouldPlay(now: number, last: number | null, gap = MIN_GAP_MS): boolean {
    return last === null || now - last >= gap
}

let audio: HTMLAudioElement | null = null
let last: number | null = null

function element(): HTMLAudioElement | null {
    if (typeof Audio === 'undefined') return null
    if (!audio) {
        audio = new Audio()
        // Opus in Ogg where the browser takes it, MP3 otherwise.
        const ogg = audio.canPlayType('audio/ogg; codecs="opus"') !== ''
        audio.src = ogg ? '/notify-signature.ogg' : '/notify-signature.mp3'
        audio.volume = VOLUME
        audio.preload = 'auto'
    }
    return audio
}

/** Plays the notification sound. Never throws: a browser may refuse before the first gesture. */
export function playNotify(): void {
    const now = Date.now()
    if (!shouldPlay(now, last)) return
    const el = element()
    if (!el) return
    last = now
    try {
        el.currentTime = 0
        void el.play().catch(() => {})
    } catch {
        /* no audio output, or blocked */
    }
}
