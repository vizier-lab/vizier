// A ```mermaid fence drawn as a diagram.
//
// The library is large, so it is imported the first time a diagram is met and never with the
// app. A diagram carries its colours in its own markup, so it is drawn from the design tokens
// and redrawn when the appearance changes (`useDark`). Anything that goes wrong — a parse
// error, a diagram this browser cannot draw — falls back to the source block, which is the
// text the reply actually said.
import { CodeBlock } from '@vizier/design'
import { memo, useEffect, useRef, useState } from 'react'
import { useDark } from '../../lib/useDark'

/** Render ids have to be unique and start with a letter; mermaid adds one element per render. */
let sequence = 0
/** The appearance the current global config was built for, so it is rebuilt only when it moves. */
let configured: string | null = null

function token(name: string): string {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim()
}

async function draw(source: string, dark: boolean): Promise<string> {
    const mermaid = (await import('mermaid')).default
    const look = dark ? 'dark' : 'light'
    if (configured !== look) {
        mermaid.initialize({
            startOnLoad: false,
            // Labels are sanitised rather than trusted: a diagram comes from model output.
            securityLevel: 'strict',
            theme: 'base',
            fontFamily: token('--font-sans'),
            themeVariables: {
                darkMode: dark,
                background: token('--vz-ground'),
                primaryColor: token('--vz-raised'),
                primaryTextColor: token('--vz-text'),
                primaryBorderColor: token('--vz-border-strong'),
                secondaryColor: token('--vz-surface'),
                secondaryTextColor: token('--vz-text-soft'),
                secondaryBorderColor: token('--vz-border'),
                tertiaryColor: token('--vz-panel'),
                tertiaryTextColor: token('--vz-text-soft'),
                tertiaryBorderColor: token('--vz-border'),
                mainBkg: token('--vz-raised'),
                nodeBorder: token('--vz-border-strong'),
                clusterBkg: token('--vz-surface'),
                clusterBorder: token('--vz-border'),
                lineColor: token('--vz-border-strong'),
                textColor: token('--vz-text'),
                edgeLabelBackground: token('--vz-ground'),
            },
        })
        configured = look
    }
    const { svg } = await mermaid.render(`vizier-mermaid-${++sequence}`, source)
    return svg
}

/** A mermaid fence: the drawn diagram, or its source while it is being drawn or if it cannot be. */
export const Mermaid = memo(function Mermaid({ code }: { code: string }) {
    const dark = useDark()
    const [svg, setSvg] = useState<string | null>(null)
    // A different diagram under the same component is drawn from scratch; the same diagram is
    // kept on screen while a theme change redraws it, so the block does not flash its source.
    const drawn = useRef(code)

    useEffect(() => {
        if (drawn.current !== code) {
            drawn.current = code
            setSvg(null)
        }
        let live = true
        draw(code, dark).then(
            (markup) => {
                if (live) setSvg(markup)
            },
            () => {
                if (live) setSvg(null)
            }
        )
        return () => {
            live = false
        }
    }, [code, dark])

    if (!svg) return <CodeBlock code={code} language="mermaid" />
    return (
        // mermaid sanitises its own output (securityLevel: strict) and produces svg.
        <div className="flex justify-center py-4 [&_svg]:h-auto [&_svg]:max-w-full" dangerouslySetInnerHTML={{ __html: svg }} />
    )
})
