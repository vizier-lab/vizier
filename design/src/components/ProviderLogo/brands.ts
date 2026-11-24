/**
 * Provider brand tiles: a letter on the provider's colour. Brand colours are
 * the one exception to "tokens only" (FR-015), and live only in this file.
 * Unknown providers (custom, local) get the neutral tile.
 */
export type ProviderBrand = { letters: string; bg: string; fg: string };

export const providerBrands: Record<string, ProviderBrand> = {
  anthropic: { letters: "A", bg: "#d97757", fg: "#ffffff" },
  openai: { letters: "O", bg: "#10a37f", fg: "#ffffff" },
  chatgpt: { letters: "C", bg: "#111111", fg: "#ffffff" },
  google: { letters: "G", bg: "#4285f4", fg: "#ffffff" },
  openrouter: { letters: "R", bg: "#6566f1", fg: "#ffffff" },
  "github-copilot": { letters: "Gh", bg: "#24292f", fg: "#ffffff" },
  mistral: { letters: "M", bg: "#fa520f", fg: "#ffffff" },
  deepseek: { letters: "D", bg: "#4d6bfe", fg: "#ffffff" },
  xai: { letters: "X", bg: "#111111", fg: "#ffffff" },
};

/** Tile letters are logo art, not text styles: Schibsted Grotesk Bold at these sizes. */
export const tileLetterSize = { sm: 10, md: 14 } as const;
