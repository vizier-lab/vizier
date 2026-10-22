/**
 * Integration logos: the tile colour and glyph of each integration kind, from
 * vizier's assets/integrations/*.svg (64-unit grid, 14-unit corner). Brand
 * colours are the one exception to "tokens only" (FR-015) and live only here.
 */

export type IntegrationBrand = {
  name: string;
  bg: string;
  /** Inner SVG markup (without the tile), drawn in white on the 64-unit grid. */
  glyph: string;
};

export const integrationBrands: Record<string, IntegrationBrand> = {
  discord: { name: "Discord", bg: "#5865f2", glyph: `<path d="M18 22c4-3 8-4 9-4l1 2c3-.5 5-.5 8 0l1-2c1 0 5 1 9 4 3 6 4 12 4 18-3 3-7 4-9 4l-2-3c1-.4 2-1 3-1.6-6 3-14 3-20 0 1 .6 2 1.2 3 1.6l-2 3c-2 0-6-1-9-4 0-6 1-12 4-18z" fill="#fff"/><circle cx="26" cy="34" r="3.2" fill="#5865f2"/><circle cx="38" cy="34" r="3.2" fill="#5865f2"/>` },
  telegram: { name: "Telegram", bg: "#2aabee", glyph: `<path d="M14 31.5 47 18.5c1.6-.6 3 .4 2.5 2.8l-5.6 26.4c-.4 1.9-1.6 2.3-3.2 1.4l-8.6-6.3-4.2 4c-.5.5-.9.9-1.8.9l.6-8.8 16-14.5c.7-.6-.2-1-1.1-.4L21.8 36.4l-8.5-2.7c-1.8-.6-1.9-1.8.7-2.2z" fill="#fff"/>` },
  brave_search: { name: "Brave Search", bg: "#fb542b", glyph: `<circle cx="28" cy="28" r="13" fill="none" stroke="#fff" stroke-width="5"/><path d="M37.5 37.5 50 50" stroke="#fff" stroke-width="6" stroke-linecap="round"/>` },
  mcp: { name: "MCP", bg: "#1f2937", glyph: `<path d="M20 38 36 22a5.7 5.7 0 0 1 8 8L30 44M26 32l10-10a5.7 5.7 0 0 1 8 8M28 46l14-14a5.7 5.7 0 0 0-8-8" fill="none" stroke="#fff" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round"/>` },
};
