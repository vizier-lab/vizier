/**
 * The live voice line, inside the composer's meta row (`specs/055-live-voice`, surface B1-1).
 *
 * One component, two shapes. Idle is a single pill — live voice is never "just there", it is
 * switched on. Live is a row that borrows the meta line the composer already renders, so the
 * window does not grow by a single pixel: the state word takes the slot the pill occupied, and
 * the context ring and the model chip are what step aside (FR-049's order).
 *
 * Geometry comes from the Lead's measured contract, not from the design sheet: the row is 24 px
 * tall and its items never shrink — only the waveform gives way, down to twelve bars and then
 * out. Colour comes from tokens, never from a render's hex.
 */

import { useEffect, useRef, useState } from 'react'
import type { LiveVoiceView } from '../../lib/liveVoice/session'

export interface LiveVoiceRowProps {
    view: LiveVoiceView
    /**
     * The agent the call is with (FR-044).
     *
     * Required by the requirement to be **in the row**, not implied by the conversation header:
     * `spec.md:572` settles that reading with the same standard FR-038 applies to the microphone —
     * a header, like a tooltip, is a different surface. Absent is allowed (a surface with no agent
     * behind it, a test), and then nothing is drawn: an empty slot that says "the agent" would be
     * worse than no slot.
     */
    agentName?: string
    /** The agent detail's `live_voice`: the row is offered only when both halves exist (FR-004). */
    capability: boolean
    /** Which half is missing, when the capability is false. */
    missing?: 'recognizer' | 'voice' | null
    /** Seconds the session has been live, shown as `m:ss`. Owned by the caller's tick. */
    elapsedSeconds?: number
    onStart(): void
    onEnd(): void
    /** `Start again` after an ending, or `Try again` after a failure. */
    onRestart(): void
}

/** Row geometry, from the measured contract. Values are px and never scale with the window. */
const ROW = { height: 24, pill: 106, end: 54, state: 70, timer: 34, micGlyph: 10, font: 10.5, gap: 8, pad: 10 }
/** The waveform: bars of 2 px with a 6 px gap, at most 39 and at least 12 before it disappears. */
const BAR = { width: 2, gap: 6, min: 12, max: 39, default: 24 }
/**
 * The agent's name in the row: at most 96 px, truncated with `…`, never wrapped (FR-044, FR-049).
 *
 * The number is the measured handoff, not a guess: 96 px is where the model chip's own floor sits,
 * so the two surrender at the same width, and 96 px at this font leaves the transcript 408 px in a
 * 1280 pane — far above the ~120 px at which a truncated line stops reading as speech. A name is
 * worth a word, not the sentence.
 *
 * `shrink: 900` is what makes it the first thing to go when the pane narrows: the fixed items
 * (state, mic, timer, End) never shrink, and against the middle slot's `flex: 1` a factor this size
 * takes the name to zero before the slot loses a pixel. At the row's 292 px floor the middle slot
 * has 16 px left, so the name is **absent rather than truncated** — which is why the test asserts
 * it above the floor and never at it (T094 measures the floor in a browser; a server render has no
 * layout to measure).
 */
const AGENT = { maxWidth: 96, endsWith: '\u2026' }

/** The word the row shows, one per state; `waiting` is FR-042's thinking, never `reconnecting`. */
export function rowWord(view: LiveVoiceView): string {
    switch (view.state) {
        case 'connecting':
            return 'Connecting…'
        case 'waiting':
            return 'Thinking'
        case 'listening':
            return 'Listening'
        case 'speaking':
            return 'Speaking'
        case 'interrupted':
            return 'Interrupted'
        case 'reconnecting':
            return 'Reconnecting'
        case 'ended':
            return 'Ended'
        case 'failed':
            return 'Failed'
        default:
            return 'Live voice'
    }
}

/**
 * The colour of the state word. Thinking is the emerald of a live session and reconnecting is
 * the amber of a recovery, so the two can never be told apart by wording alone becoming the only
 * cue (FR-042) — and thinking carries its own marker, three dots, which survives reduced motion.
 */
export function rowTone(view: LiveVoiceView): string {
    switch (view.state) {
        case 'reconnecting':
            return 'text-retrying-text'
        case 'ended':
            return 'text-muted'
        case 'failed':
            return 'text-failed-text'
        case 'waiting':
        case 'listening':
        case 'speaking':
        case 'interrupted':
        case 'connecting':
            // The word, not the mark: state/live is 3.77:1 on white and 3.43:1 on bg-surface,
            // under the 4.5:1 a 10.5 px word needs. state/live-text is 6.44:1 and 5.86:1, and the
            // dark theme resolves both to the same emerald, so nothing changes there.
            return 'text-live-text'
        default:
            return 'text-faint'
    }
}

function clock(totalSeconds: number): string {
    const safe = Math.max(0, Math.floor(totalSeconds))
    return `${Math.floor(safe / 60)}:${String(safe % 60).padStart(2, '0')}`
}

/** The microphone mark. Small, deliberate, and only ever drawn beside its own word. */
function MicGlyph({ muted }: { muted: boolean }) {
    return (
        <svg
            width={ROW.micGlyph}
            height={13}
            viewBox="0 0 10 13"
            aria-hidden
            className={muted ? 'text-faint' : 'text-live'}
        >
            <rect x="3.5" y="0.5" width="3" height="7" rx="1.5" fill="currentColor" />
            <path
                d="M1 6.5a4 4 0 0 0 8 0M5 10.5V12.5"
                fill="none"
                stroke="currentColor"
                strokeWidth="1"
                strokeLinecap="round"
            />
        </svg>
    )
}

/**
 * The waveform, measured rather than assumed. A server render has no layout, so it starts at the
 * default and the real count arrives from the container: `floor((width + gap) / (bar + gap))`,
 * clamped so it is never unreadably dense and never disappears before the minimum.
 */
function Waveform({ level }: { level: number }) {
    const ref = useRef<HTMLDivElement | null>(null)
    const [bars, setBars] = useState(BAR.default)

    useEffect(() => {
        const element = ref.current
        if (!element || typeof ResizeObserver === 'undefined') return
        const measure = () => {
            const fit = Math.floor((element.clientWidth + BAR.gap) / (BAR.width + BAR.gap))
            setBars(Math.min(BAR.max, Math.max(BAR.min, fit)))
        }
        measure()
        const observer = new ResizeObserver(measure)
        observer.observe(element)
        return () => observer.disconnect()
    }, [])

    const loud = Math.max(0, Math.min(1, level))
    return (
        <div ref={ref} data-bars={bars} aria-hidden className="flex min-w-0 flex-1 items-center" style={{ gap: BAR.gap }}>
            {Array.from({ length: bars }, (_, index) => {
                // A fixed shape, taller in the middle, nudged by the input level: enough to read as
                // "this is live" without pretending to be a real spectrum.
                const shape = 0.35 + 0.45 * Math.sin((index / Math.max(1, bars - 1)) * Math.PI)
                const height = Math.round(4 + (shape + loud * 0.35) * 12)
                return (
                    <span
                        key={index}
                        className="shrink-0 rounded-full bg-live"
                        style={{ width: BAR.width, height: Math.min(16, height) }}
                    />
                )
            })}
        </div>
    )
}

export function LiveVoiceRow({
    view,
    agentName,
    capability,
    missing,
    elapsedSeconds = 0,
    onStart,
    onEnd,
    onRestart,
}: LiveVoiceRowProps) {
    const idle = view.state === 'off' || view.state === 'ended' || view.state === 'failed'
    const terminal = view.state === 'ended' || view.state === 'failed'

    // FR-004: a control nobody can use says which half is missing, where the settings are.
    if (!capability) {
        const why =
            missing === 'voice'
                ? 'a voice is not configured for this agent'
                : missing === 'recognizer'
                  ? 'a recogniser is not configured for this agent'
                  : 'live voice is not configured for this agent'
        return (
            <span
                title={why}
                className="inline-flex items-center whitespace-nowrap rounded-full border border-border text-faint"
                style={{ height: ROW.height, fontSize: ROW.font, gap: ROW.gap, paddingInline: ROW.pad }}
            >
                <MicGlyph muted />
                <span>Live voice</span>
                <span className="sr-only">{why}</span>
            </span>
        )
    }

    if (view.state === 'off') {
        return (
            <button
                type="button"
                onClick={onStart}
                aria-label="Start live voice"
                className="inline-flex items-center justify-center whitespace-nowrap rounded-full border border-border text-muted hover:bg-raised hover:text-fg"
                style={{ width: ROW.pill, height: ROW.height, fontSize: ROW.font, gap: ROW.gap }}
            >
                <MicGlyph muted={false} />
                <span>Live voice</span>
            </button>
        )
    }

    const reason = view.notice ?? (terminal ? rowWord(view).toLowerCase() : null)
    // Trimmed, not shown as given: a name of spaces is no name, and the slot it would take comes
    // out of the transcript.
    const name = agentName?.trim() ?? ''

    return (
        <div
            role="group"
            aria-label="Live voice"
            className="flex min-w-0 items-center text-faint"
            style={{ height: ROW.height, fontSize: ROW.font, gap: ROW.gap }}
        >
            <span
                role="status"
                aria-live="polite"
                data-state={view.state}
                className={`shrink-0 whitespace-nowrap font-medium ${rowTone(view)}`}
                style={{ width: ROW.state }}
            >
                {rowWord(view)}
            </span>

            {/* The thinking marker: three dots, still there when animation is switched off. */}
            <span aria-hidden className={`shrink-0 whitespace-nowrap ${rowTone(view)}`}>
                {view.state === 'waiting' ? '•••' : view.state === 'reconnecting' ? '•' : ''}
            </span>

            {/* FR-044: the agent the call is with, in the row. `title` carries the whole name for
                the case the ellipsis hides it; the row's own text is the name itself, because a
                tooltip is not "show" (the reading FR-038 fixes for the microphone). */}
            {name ? (
                <span
                    data-agent
                    title={name}
                    className="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-soft"
                    style={{ maxWidth: AGENT.maxWidth, flexShrink: 900, flexBasis: 'auto' }}
                >
                    {name}
                </span>
            ) : null}

            <span className="inline-flex shrink-0 items-center whitespace-nowrap text-faint" style={{ gap: 4 }}>
                <MicGlyph muted={!view.micOpen} />
                <span>{view.micOpen ? 'mic on' : 'mic off'}</span>
            </span>

            {/* The middle slot, which costs no height: the reason when the call is over, the
                words that were heard while it runs, the waveform otherwise. The transcript used to
                live only in `LiveVoiceBar`, which stopped being rendered when the row replaced it —
                so it was in state and never on screen, and a person saw it only after a refresh,
                as the stored message (F4). */}
            {idle && reason ? (
                <span className="min-w-0 flex-1 truncate whitespace-nowrap text-faint" title={reason}>
                    {reason}
                </span>
            ) : view.transcript ? (
                <span
                    data-transcript
                    className="min-w-0 flex-1 truncate whitespace-nowrap text-soft"
                    title={view.transcript}
                >
                    {view.transcript}
                </span>
            ) : (
                <Waveform level={view.level} />
            )}

            <span className="shrink-0 whitespace-nowrap tabular-nums text-faint" style={{ width: ROW.timer }}>
                {clock(elapsedSeconds)}
            </span>

            {terminal ? (
                <button
                    type="button"
                    onClick={onRestart}
                    className="shrink-0 whitespace-nowrap rounded-full bg-live font-medium text-ground"
                    style={{ height: ROW.height, fontSize: ROW.font, paddingInline: ROW.pad }}
                >
                    {view.state === 'failed' ? 'Try again' : 'Start again'}
                </button>
            ) : (
                <button
                    type="button"
                    onClick={onEnd}
                    aria-label="End the live voice session"
                    className="shrink-0 whitespace-nowrap rounded-full bg-failed font-medium text-ground"
                    style={{ width: ROW.end, height: ROW.height, fontSize: ROW.font, paddingInline: 0 }}
                >
                    End
                </button>
            )}
        </div>
    )
}
