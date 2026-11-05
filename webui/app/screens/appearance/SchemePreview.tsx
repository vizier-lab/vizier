import { Button, CodeBlock, NavItem, Status, ThemeScope, type CodeLine } from '@vizier/design'
import { allVars } from '../../lib/themes/apply'
import type { Appearance, Variant } from '../../lib/themes/roles'

const SAMPLE_CODE = `# find what the agent remembers
def recall(query):
    hits = memory_search(query, limit=5)
    return hits or "nothing yet"
`

const SAMPLE_TOKENS: CodeLine[] = [
    [{ text: '# find what the agent remembers', tone: 'comment' }],
    [
        { text: 'def', tone: 'keyword' },
        { text: ' ' },
        { text: 'recall', tone: 'function' },
        { text: '(', tone: 'punctuation' },
        { text: 'query' },
        { text: '):', tone: 'punctuation' },
    ],
    [
        { text: '    hits ' },
        { text: '=', tone: 'punctuation' },
        { text: ' ' },
        { text: 'memory_search', tone: 'function' },
        { text: '(', tone: 'punctuation' },
        { text: 'query' },
        { text: ',', tone: 'punctuation' },
        { text: ' limit' },
        { text: '=', tone: 'punctuation' },
        { text: '5', tone: 'number' },
        { text: ')', tone: 'punctuation' },
    ],
    [
        { text: '    ' },
        { text: 'return', tone: 'keyword' },
        { text: ' hits ' },
        { text: 'or', tone: 'keyword' },
        { text: ' ' },
        { text: '"nothing yet"', tone: 'string' },
    ],
]

/** The real components, drawn in one variant's colours whatever the page itself is showing. */
export function SchemePreview({ variant, appearance }: { variant: Variant; appearance: Appearance }) {
    return (
        <ThemeScope vars={allVars(variant.roles)} appearance={appearance} className="flex overflow-hidden rounded-xl border border-border bg-shell" inert>
            <div className="hidden w-128 shrink-0 flex-col gap-2 p-8 sm:flex">
                <NavItem icon="chat" label="Chat" active href="#" />
                <NavItem icon="memory" label="Memory" href="#" />
                <NavItem icon="tasks" label="Tasks" href="#" />
            </div>
            <div className="m-8 flex min-w-0 flex-1 flex-col gap-12 rounded-lg border border-border bg-panel p-12 sm:ml-0">
                <div className="flex flex-wrap items-center gap-12">
                    <Status state="live" label="Live" />
                    <Status state="retrying" label="Retrying" />
                    <Status state="failed" label="Failed" />
                </div>
                <CodeBlock code={SAMPLE_CODE} tokens={SAMPLE_TOKENS} language="python" />
                <div className="flex flex-wrap items-center gap-12">
                    <Button variant="primary">Send</Button>
                    <span className="type-small text-faint">Faint helper text</span>
                </div>
            </div>
        </ThemeScope>
    )
}
