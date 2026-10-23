import { useEffect, useState } from 'react'

/** The design's phone layout applies below this width (Tailwind's `lg`). */
export const MOBILE_QUERY = '(max-width: 1023px)'

const match = () =>
    typeof window !== 'undefined' && window.matchMedia(MOBILE_QUERY).matches

/** Whether the screen is phone-sized, following the window as it resizes. */
export function useMobile(): boolean {
    const [mobile, setMobile] = useState(match)
    useEffect(() => {
        const query = window.matchMedia(MOBILE_QUERY)
        const on = () => setMobile(query.matches)
        on()
        query.addEventListener('change', on)
        return () => query.removeEventListener('change', on)
    }, [])
    return mobile
}
