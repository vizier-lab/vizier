// A skill's description is a paragraph written for a catalogue page; the slash menu gives it one line
// beside the name. Take its first sentence, and cut that at a word if it is still too long, so a row
// ends on a word ("… otherwise improve a frontend interface") rather than mid-word.

// The end of a first sentence: a full stop, question or bang before a capital or the end of the text.
// "e.g. something" is not a sentence end, because what follows is lowercase.
const SENTENCE_END = /[.!?](?=\s+[A-Z]|\s*$)/

/** The description as a menu row draws it: one sentence, at most `limit` characters. */
export function skillDescription(text: string, limit = 110): string {
    // Frontmatter paragraphs are hard-wrapped; a row must not inherit those line ends.
    const flat = text.replace(/\s+/g, ' ').trim()
    if (!flat) return ''
    const end = flat.search(SENTENCE_END)
    const sentence = end < 0 ? flat : flat.slice(0, end + 1)
    if (sentence.length <= limit) return sentence
    const cut = sentence.slice(0, limit)
    const space = cut.lastIndexOf(' ')
    const kept = (space > 0 ? cut.slice(0, space) : cut).replace(/[,;:\-–—]+$/, '')
    return `${kept}…`
}
