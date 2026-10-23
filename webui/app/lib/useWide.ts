import { useEffect, useState } from 'react'

/** Room for a list and a detail pane side by side (Tailwind's `xl`). */
const QUERY = '(min-width: 1280px)'

const match = () => typeof window !== 'undefined' && window.matchMedia(QUERY).matches

export function useWide(): boolean {
    const [wide, setWide] = useState(match)
    useEffect(() => {
        const query = window.matchMedia(QUERY)
        const on = () => setWide(query.matches)
        on()
        query.addEventListener('change', on)
        return () => query.removeEventListener('change', on)
    }, [])
    return wide
}
