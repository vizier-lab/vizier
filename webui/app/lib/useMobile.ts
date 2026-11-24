import { useEffect, useState } from 'react'

/** The design's phone layout applies below this width (Tailwind's `lg`). */
export const MOBILE_QUERY = '(max-width: 1023px)'

const matches = (query: string) =>
    typeof window !== 'undefined' && window.matchMedia(query).matches

/** A media query, followed as the window resizes. */
function useMedia(query: string): boolean {
    const [on, setOn] = useState(() => matches(query))
    useEffect(() => {
        const q = window.matchMedia(query)
        const update = () => setOn(q.matches)
        update()
        q.addEventListener('change', update)
        return () => q.removeEventListener('change', update)
    }, [query])
    return on
}

/** Whether the screen is phone-sized, following the window as it resizes. */
export const useMobile = () => useMedia(MOBILE_QUERY)

/**
 * Whether the window is narrower than `maxWidth` px. The step between the phone layout and the
 * wide one: a page that needs three columns on a big screen stacks them here instead.
 */
export const useBelow = (maxWidth: number) => useMedia(`(max-width: ${maxWidth - 1}px)`)
