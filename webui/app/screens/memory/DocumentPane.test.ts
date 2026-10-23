import { expect, it } from 'vitest'
import { linkWikiLinks, relationLink } from './DocumentPane'

it('links wiki links into Memory, in this bundle or another', () => {
    expect(relationLink('plug', 'infra', 'qdrant.md')).toMatchObject({ href: '/plug/memory/infra?doc=qdrant' })
    expect(relationLink('plug', 'infra', 'ops/runbook')).toMatchObject({ href: '/plug/memory/ops?doc=runbook', note: 'ops' })
    expect(linkWikiLinks('plug', 'infra', 'See [[ops/runbook]] and [[ops]] and [[ops/runbook|the runbook]].')).toBe(
        'See [ops/runbook](/plug/memory/ops?doc=runbook) and [ops](/plug/memory/ops) and [the runbook](/plug/memory/ops?doc=runbook).'
    )
})

it('leaves code alone', () => {
    const md = 'Use `[[x/y]]` or\n```\n[[x/y]]\n```\nnot.'
    expect(linkWikiLinks('plug', 'infra', md)).toBe(md)
})
