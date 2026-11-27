import { expect, it } from 'vitest'
import { linkWikiLinks, relationLink } from './memoryLinks'
import { rewriteEntityLinks } from './entityLinks'

it('links wiki links into Memory, in this bundle or another', () => {
    expect(relationLink('plug', 'infra', 'qdrant.md')).toMatchObject({ href: '/plug/memory/infra/qdrant' })
    expect(relationLink('plug', 'infra', 'ops/runbook')).toMatchObject({ href: '/plug/memory/ops/runbook', note: 'ops' })
    expect(linkWikiLinks('plug', 'infra', 'See [[ops/runbook]] and [[ops]] and [[ops/runbook|the runbook]].')).toBe(
        'See [ops/runbook](/plug/memory/ops/runbook) and [ops](/plug/memory/ops) and [the runbook](/plug/memory/ops/runbook).'
    )
})

it('leaves code alone', () => {
    const md = 'Use `[[x/y]]` or\n```\n[[x/y]]\n```\nnot.'
    expect(linkWikiLinks('plug', 'infra', md)).toBe(md)
})

it('leaves entity links for the entity rewrite, which then links them to their page', () => {
    const md = 'Cites [[artifact:report]], [[function:fetch_weather|the fetcher]] and [[ops]].'
    expect(linkWikiLinks('plug', 'infra', md)).toBe(
        'Cites [[artifact:report]], [[function:fetch_weather|the fetcher]] and [ops](/plug/memory/ops).'
    )
    expect(rewriteEntityLinks('plug', linkWikiLinks('plug', 'infra', md))).toBe(
        'Cites [report](/plug/artifacts?artifact=report), [the fetcher](/plug/functions/fetch_weather) and [ops](/plug/memory/ops).'
    )
})
