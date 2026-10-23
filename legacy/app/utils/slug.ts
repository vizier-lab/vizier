/**
 * Auto-correction and normalization for slug inputs
 * Converts any input into a valid slug format
 */

/**
 * Correct slug input while user is typing (lenient - allows trailing hyphens)
 * This allows "hello-" to remain as "hello-" so user can continue typing
 * 
 * Rules applied:
 * - Convert to lowercase
 * - Replace spaces with hyphens
 * - Replace consecutive hyphens with single
 * - Remove special characters (keep only alphanumeric, hyphens, underscores)
 * - Only remove LEADING hyphens (not trailing, to allow typing)
 * 
 * @param input The raw input string
 * @returns Corrected slug string (lenient)
 */
export function autoCorrectSlug(input: string): string {
  if (!input) return ''

  return (
    input
      .toLowerCase() // Convert to lowercase
      .trim() // Remove leading/trailing whitespace
      .replace(/\s+/g, '-') // Replace spaces with hyphens
      .replace(/[^a-z0-9_-]/g, '') // Remove special characters (keep alphanumeric, underscores, hyphens)
      .replace(/-+/g, '-') // Replace consecutive hyphens with single hyphen
      .replace(/^-+/g, '') // Remove ONLY leading hyphens (allow trailing for live input)
  )
}

/**
 * Final slug validation - strict normalization for submission
 * Removes both leading and trailing hyphens
 * 
 * @param input The raw input string
 * @returns Corrected slug string (strict)
 */
export function autoCorrectSlugStrict(input: string): string {
  if (!input) return ''

  return autoCorrectSlug(input)
    .replace(/-+$/g, '') // Remove trailing hyphens
}

/**
 * Validate if a slug is in correct format
 * @param slug The slug to validate
 * @returns true if slug is valid
 */
export function isValidSlug(slug: string): boolean {
  if (!slug || typeof slug !== 'string') return false
  const validSlugRegex = /^[a-z0-9][a-z0-9_-]*[a-z0-9]$|^[a-z0-9]$/
  return validSlugRegex.test(slug)
}

/**
 * Get a suggestion message if the corrected slug differs from input
 * @param input Original input
 * @param corrected Corrected slug
 * @returns Suggestion message or null if no correction needed
 */
export function getSlugSuggestion(input: string, corrected: string): string | null {
  if (input === corrected || !corrected) return null
  return `Slug corrected: "${corrected}"`
}
