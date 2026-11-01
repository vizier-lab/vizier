import { useEffect, useState } from 'react'

/** The appearance showing now: the stored choice if there is one, else the system's. */
export function isDark(): boolean {
    // Nothing is rendered yet on the server or in a test without a DOM: light is the safe answer,
    // and the first effect corrects it.
    if (typeof document === 'undefined') return false
    const attr = document.documentElement.getAttribute('data-theme')
    return attr ? attr === 'dark' : matchMedia('(prefers-color-scheme: dark)').matches
}

/**
 * Whether the page is dark, following both ways it can change: the stored choice rewriting
 * `data-theme`, and the system switching while the choice is `system`.
 *
 * For a drawing that is made in code rather than in CSS — a mermaid diagram carries colours in
 * its own markup — so it has to be redrawn when the theme moves.
 */
export function useDark(): boolean {
    const [dark, setDark] = useState(isDark)

    useEffect(() => {
        const read = () => setDark(isDark())
        const media = matchMedia('(prefers-color-scheme: dark)')
        media.addEventListener('change', read)
        const observer = new MutationObserver(read)
        observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })
        return () => {
            media.removeEventListener('change', read)
            observer.disconnect()
        }
    }, [])

    return dark
}
