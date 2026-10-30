// The one place an <a> is made for a generated interface (research R12). A link to an address
// follows the link rules; a link to a conversation file downloads it. An address the rules refuse
// is drawn as plain text, so a markdown link in a `Text` cannot smuggle a scheme in.
import type { ReactNode } from 'react'
import type { SessionFileRecord } from '../../../api/frames'
import { fileUrl } from '../media'
import { hostToDisclose, parseHref } from '../links'

export function SafeLink({ href, label }: { href: string; label: ReactNode }) {
    const link = parseHref(href)
    if (!link) return <span>{label}</span>
    const disclose = typeof label === 'string' ? hostToDisclose(label, link) : link.host
    return (
        <>
            <a href={link.href} target="_blank" rel="noopener noreferrer nofollow" referrerPolicy="no-referrer" className="text-fg underline underline-offset-2">
                {label}
            </a>
            {disclose && <span className="ml-4 type-mono-xs text-faint">({disclose})</span>}
        </>
    )
}

/** A link that downloads a file of the conversation. */
export function FileLink({ file, label }: { file: SessionFileRecord; label: ReactNode }) {
    return (
        <a href={fileUrl(file)} download={file.filename} rel="noopener noreferrer" referrerPolicy="no-referrer" className="text-fg underline underline-offset-2">
            {label}
        </a>
    )
}
