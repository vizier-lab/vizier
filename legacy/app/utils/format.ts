/** `$1.23 est.`, or `—` when no price is known (never `$0`). */
export function formatCost(usd: number | null | undefined): string {
  if (usd == null) return '—'
  if (usd > 0 && usd < 0.01) return '<$0.01 est.'
  return `$${usd.toFixed(2)} est.`
}

/** `1.2M`, `340K`, `912`. */
export function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${Math.round(n / 1_000)}K`
  return String(Math.round(n))
}

export function formatPercent(fraction: number | null | undefined, digits = 0): string {
  if (fraction == null) return '—'
  return `${(fraction * 100).toFixed(digits)}%`
}

export function formatCount(n: number): string {
  return n.toLocaleString('en-US')
}
