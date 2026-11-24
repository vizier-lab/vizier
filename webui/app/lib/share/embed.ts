/**
 * Serializes a value or string as JSON suitable for embedding inside a <script type="application/json">
 * tag in an HTML document without risking script breakout, CDATA breakout or encoding anomalies.
 *
 * Replaces:
 * - '<' with '\u003c'
 * - '>' with '\u003e'
 * - '&' with '\u0026'
 * - '\u2028' (LINE SEPARATOR) with '\u2028'
 * - '\u2029' (PARAGRAPH SEPARATOR) with '\u2029'
 */
export function embedJson(value: unknown): string {
    const json = typeof value === 'string' ? value : JSON.stringify(value)
    return json
        .replace(/</g, '\\u003c')
        .replace(/>/g, '\\u003e')
        .replace(/&/g, '\\u0026')
        .replace(/\u2028/g, '\\u2028')
        .replace(/\u2029/g, '\\u2029')
}

export function escapeHtml(text: string): string {
    return text
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;')
}
