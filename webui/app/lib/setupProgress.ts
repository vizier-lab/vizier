// Progress of an agent's setup, from the downloads its build reports.
import type { SetupState } from '../api/endpoints/agents'

export type SetupSample = { at: number; progress: number }

/** Recent samples kept per starting agent, enough for a steady rate. */
export const SAMPLES = 5

/** 0..1 from a setup's downloads, or null when it reports none. */
export function setupProgress(s: SetupState): number | null {
    const ds = s.downloads.filter(
        (d) =>
            (typeof d.progress === 'number' && d.progress >= 0) ||
            (typeof d.completed === 'number' && typeof d.total === 'number' && d.total > 0)
    )
    if (!ds.length) return null
    return (
        ds.reduce((a, d) => {
            if (typeof d.progress === 'number') return a + Math.min(1, Math.max(0, d.progress))
            return a + (d.completed ?? 0) / (d.total ?? 1)
        }, 0) / ds.length
    )
}

/** Seconds left, extrapolated from the recent samples; null until there are two that moved. */
export function eta(samples: SetupSample[] | undefined): number | null {
    if (!samples || samples.length < 2) return null
    const a = samples[0]
    const b = samples[samples.length - 1]
    const rate = (b.progress - a.progress) / ((b.at - a.at) / 1000)
    if (!(rate > 0)) return null
    return Math.ceil((1 - b.progress) / rate)
}
