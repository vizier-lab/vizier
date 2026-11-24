import { expect, it } from 'vitest'
import { estimateCost } from './prices'

it('prices fresh, cached and written input and output separately', () => {
    const stats = { total_input_tokens: 1_000_000, total_cached_input_tokens: 400_000, total_cache_creation_input_tokens: 100_000, total_output_tokens: 100_000 }
    const usd = estimateCost(stats, { input: 3, output: 15, cache_read: 0.3, cache_write: 3.75 })
    // 500k*3 + 400k*0.3 + 100k*3.75 + 100k*15 = 1.5 + 0.12 + 0.375 + 1.5 = 3.495
    expect(usd).toBeCloseTo(3.495, 6)
})

it('falls back to the input price when the cache has none', () => {
    const stats = { total_input_tokens: 1_000_000, total_cached_input_tokens: 500_000, total_cache_creation_input_tokens: 0, total_output_tokens: 0 }
    expect(estimateCost(stats, { input: 2, output: 8 })).toBeCloseTo(2, 6)
})
