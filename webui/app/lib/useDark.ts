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

/** Everything that decides the colours on screen, as one string. */
function appearanceKey(): string {
    if (typeof document === 'undefined') return ''
    const html = document.documentElement
    return [html.getAttribute('data-theme'), html.getAttribute('style'), matchMedia('(prefers-color-scheme: dark)').matches].join('|')
}

/**
 * A string that changes whenever the colours on screen change: a scheme, a variant or the device. For
 * a drawing made in code from resolved colours; `useDark` stays for code that only needs light or dark.
 */
export function useAppearanceKey(): string {
    const [key, setKey] = useState(appearanceKey)

    useEffect(() => {
        const read = () => setKey(appearanceKey())
        const media = matchMedia('(prefers-color-scheme: dark)')
        media.addEventListener('change', read)
        const observer = new MutationObserver(read)
        observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'style'] })
        return () => {
            media.removeEventListener('change', read)
            observer.disconnect()
        }
    }, [])

    return key
}
