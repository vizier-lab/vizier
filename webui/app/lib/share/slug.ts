const RESERVED_WINDOWS_NAMES = /^(con|prn|aux|nul|com[1-9]|lpt[1-9])$/i

/**
 * Derives a clean file name for an exported snapshot:
 * `<slug of title, ≤60 chars, [a-z0-9-]>-<yyyymmdd>.vizier.html`
 * Fallback when empty or Windows reserved name: `conversation-<yyyymmdd>.vizier.html`
 */
export function exportFileName(title: string, date: Date = new Date()): string {
    let slug = title
        .normalize('NFKD')
        .toLowerCase()
        .replace(/[\\/:*?"<>|]/g, '-')
        .replace(/[^a-z0-9-]/g, '-')
        .replace(/-+/g, '-')
        .replace(/^-+|-+$/g, '')

    slug = slug.slice(0, 60).replace(/-+$/, '')

    if (!slug || RESERVED_WINDOWS_NAMES.test(slug)) {
        slug = 'conversation'
    }

    const yyyy = date.getFullYear()
    const mm = String(date.getMonth() + 1).padStart(2, '0')
    const dd = String(date.getDate()).padStart(2, '0')
    const yyyymmdd = `${yyyy}${mm}${dd}`

    return `${slug}-${yyyymmdd}.vizier.html`
}
