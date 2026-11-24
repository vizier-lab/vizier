// What the registry components share: the closed enums mapped to the design library's values,
// and the one way an event is emitted, so every action chain knows whether a person started it.
import { useBoundProp, useStateStore } from '@json-render/react'
import { useCallback, useState } from 'react'
import { noteGesture } from '../trust'

export const GAPS = { none: 0, sm: 8, md: 12, lg: 24 } as const
export type GapName = keyof typeof GAPS

export const gapOf = (name: unknown): 0 | 8 | 12 | 24 => GAPS[(name as GapName) in GAPS ? (name as GapName) : 'md']

export type ToneName = 'default' | 'accent' | 'ok' | 'warn' | 'error'

/** What an event handler gets: `emit(name)` plus the originating DOM event, if any. */
export type Emit = (event: string) => void

/**
 * Emit `name` from a DOM event (`source`) or from none (a timer, a load). The chain it starts is
 * trusted, and so may `sendMessage`, only when a real person's click, key or tap started it.
 * An event's data goes to `/_event` first, where the action's params can read it.
 */
export function useEmitter(emit: Emit) {
  const { set } = useStateStore()
  return useCallback(
    (name: string, source?: { isTrusted?: boolean } | null, payload?: unknown) => {
      noteGesture(source?.isTrusted === true)
      if (payload !== undefined) set('/_event', payload)
      // A refused or failed action stops its chain and has already been reported; nothing more to do.
      void Promise.resolve(emit(name) as unknown).catch(() => {})
    },
    [emit, set]
  )
}

/**
 * A field's value: written back to its bound state path, or, when the prop is a plain literal,
 * kept locally so the control still works.
 */
export function useField<T>(value: T | undefined, path: string | undefined, fallback: T): [T, (next: T) => void] {
  const [bound, write] = useBoundProp<T>(value, path)
  const [local, setLocal] = useState<T>(value ?? fallback)
  if (path) return [bound ?? fallback, write]
  return [local, setLocal]
}
