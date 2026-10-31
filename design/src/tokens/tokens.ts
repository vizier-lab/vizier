/**
 * Token names and descriptions, for the Foundations pages and anything that
 * needs to list tokens. Values are NOT here: they live only in tokens.css and
 * are read from CSS at runtime (getComputedStyle), so nothing is written twice.
 */

export type TokenGroup = "bg" | "line" | "dot" | "text" | "fill" | "state" | "data";

export type ColorToken = {
  /** Design name, e.g. "bg/surface". */
  name: string;
  /** CSS custom property, e.g. "--vz-surface". */
  cssVar: string;
  /** Tailwind colour name: `bg-<tw>`, `text-<tw>`, `border-<tw>`. */
  tw: string;
  group: TokenGroup;
  description: string;
};

export const colorTokens: ColorToken[] = [
  { name: "bg/ground", cssVar: "--vz-ground", tw: "ground", group: "bg", description: "Page background" },
  { name: "bg/sidebar", cssVar: "--vz-sidebar", tw: "sidebar", group: "bg", description: "Sidebar, settings rail and side panels" },
  { name: "bg/shell", cssVar: "--vz-shell", tw: "shell", group: "bg", description: "The app frame: the rail and the context sidebar" },
  { name: "bg/panel", cssVar: "--vz-panel", tw: "panel", group: "bg", description: "The inset content panel the pages sit on" },
  { name: "bg/surface", cssVar: "--vz-surface", tw: "surface", group: "bg", description: "Cards, panels, chat bubbles, trail" },
  { name: "bg/raised", cssVar: "--vz-raised", tw: "raised", group: "bg", description: "Inputs, hovered and selected rows, active nav, menus" },
  { name: "bg/pressed", cssVar: "--vz-pressed", tw: "pressed", group: "bg", description: "Secondary, ghost and row controls while pressed (hover is bg/raised)" },
  { name: "line/border", cssVar: "--vz-border", tw: "border", group: "line", description: "Dividers and card edges" },
  { name: "line/border-strong", cssVar: "--vz-border-strong", tw: "border-strong", group: "line", description: "Input and button edges, timeline spine" },
  { name: "line/focus", cssVar: "--vz-focus", tw: "focus", group: "line", description: "Keyboard focus ring (Focus/ring) on buttons, toggles, tabs, rows and menus. Text entry focus stays emerald: typing is live." },
  { name: "line/border-hover", cssVar: "--vz-border-hover", tw: "border-hover", group: "line", description: "Control edges under the pointer" },
  { name: "dot/unlit", cssVar: "--vz-unlit", tw: "unlit", group: "dot", description: "Unlit sigil dots, tracks, disabled controls" },
  { name: "dot/off", cssVar: "--vz-dot-off", tw: "dot-off", group: "dot", description: "Off dots on LED boards and activity grids" },
  { name: "text/faint", cssVar: "--vz-text-faint", tw: "faint", group: "text", description: "Timestamps, column labels, hints. Never body text" },
  { name: "text/muted", cssVar: "--vz-text-muted", tw: "muted", group: "text", description: "Secondary text, descriptions, inactive nav" },
  { name: "text/soft", cssVar: "--vz-text-soft", tw: "soft", group: "text", description: "Code, table cells, field labels" },
  { name: "text/default", cssVar: "--vz-text", tw: "fg", group: "text", description: "Body and headings" },
  { name: "fill/ink", cssVar: "--vz-ink", tw: "ink", group: "fill", description: "Lit dots, primary button fill, switch on" },
  { name: "fill/ink-hover", cssVar: "--vz-ink-hover", tw: "ink-hover", group: "fill", description: "Primary button and ink fills under the pointer" },
  { name: "fill/ink-pressed", cssVar: "--vz-ink-pressed", tw: "ink-pressed", group: "fill", description: "Primary button and ink fills while pressed" },
  { name: "state/live", cssVar: "--vz-live", tw: "live", group: "state", description: "Emerald: live only. Always paired with a word" },
  { name: "state/live-text", cssVar: "--vz-live-text", tw: "live-text", group: "state", description: "Emerald text on badges, statuses and diffs: 4.5:1 on every ground, a selected row included" },
  { name: "state/rank", cssVar: "--vz-rank", tw: "rank", group: "state", description: "Lapis: owner, superadmin, grants" },
  { name: "state/rank-text", cssVar: "--vz-rank-text", tw: "rank-text", group: "state", description: "Lapis text in badges" },
  { name: "state/rank-bg", cssVar: "--vz-rank-bg", tw: "rank-bg", group: "state", description: "Lapis badge ground" },
  { name: "state/failed", cssVar: "--vz-failed", tw: "failed", group: "state", description: "Ember: failed agents and readers, destructive actions" },
  { name: "state/failed-text", cssVar: "--vz-failed-text", tw: "failed-text", group: "state", description: "Ember text on notices and badges" },
  { name: "state/failed-bg", cssVar: "--vz-failed-bg", tw: "failed-bg", group: "state", description: "Ember notice ground" },
  { name: "state/live-bg", cssVar: "--vz-live-bg", tw: "live-bg", group: "state", description: "Added-line ground" },
  { name: "state/failed-line", cssVar: "--vz-failed-line", tw: "failed-line", group: "state", description: "Ember notice and destructive-button edge" },
  { name: "state/retrying", cssVar: "--vz-retrying", tw: "retrying", group: "state", description: "Amber: retrying with backoff, limits" },
  { name: "state/retrying-bg", cssVar: "--vz-retrying-bg", tw: "retrying-bg", group: "state", description: "Amber notice ground" },
  { name: "state/retrying-line", cssVar: "--vz-retrying-line", tw: "retrying-line", group: "state", description: "Amber notice edge" },
  { name: "state/failed-hover", cssVar: "--vz-failed-hover", tw: "failed-hover", group: "state", description: "Destructive button fill under the pointer" },
  { name: "state/failed-pressed", cssVar: "--vz-failed-pressed", tw: "failed-pressed", group: "state", description: "Destructive button fill while pressed" },
  { name: "data/graph-edge", cssVar: "--vz-graph-edge", tw: "graph-edge", group: "data", description: "Memory graph links" },
  { name: "data/graph-edge-strong", cssVar: "--vz-graph-edge-strong", tw: "graph-edge-strong", group: "data", description: "Memory graph links of the selected node" },
  { name: "data/chart-1", cssVar: "--vz-chart-1", tw: "chart-1", group: "data", description: "Chart series: dreams" },
  { name: "data/chart-2", cssVar: "--vz-chart-2", tw: "chart-2", group: "data", description: "Chart series: compaction" },
  { name: "data/chart-3", cssVar: "--vz-chart-3", tw: "chart-3", group: "data", description: "Chart series: vision" },
  { name: "data/chart-4", cssVar: "--vz-chart-4", tw: "chart-4", group: "data", description: "Chart series: background" },
  { name: "data/chart-5", cssVar: "--vz-chart-5", tw: "chart-5", group: "data", description: "Chart series: scheduled" },
];

export type SizeToken = { name: string; cssVar: string; kind: "radius" | "space" };

export const sizeTokens: SizeToken[] = [
  { name: "radius/xs", cssVar: "--vz-radius-xs", kind: "radius" },
  { name: "radius/sm", cssVar: "--vz-radius-sm", kind: "radius" },
  { name: "radius/md", cssVar: "--vz-radius-md", kind: "radius" },
  { name: "radius/lg", cssVar: "--vz-radius-lg", kind: "radius" },
  { name: "radius/xl", cssVar: "--vz-radius-xl", kind: "radius" },
  { name: "radius/2xl", cssVar: "--vz-radius-2xl", kind: "radius" },
  { name: "radius/pill", cssVar: "--vz-radius-pill", kind: "radius" },
  { name: "space/2", cssVar: "--vz-space-2", kind: "space" },
  { name: "space/4", cssVar: "--vz-space-4", kind: "space" },
  { name: "space/6", cssVar: "--vz-space-6", kind: "space" },
  { name: "space/8", cssVar: "--vz-space-8", kind: "space" },
  { name: "space/10", cssVar: "--vz-space-10", kind: "space" },
  { name: "space/12", cssVar: "--vz-space-12", kind: "space" },
  { name: "space/14", cssVar: "--vz-space-14", kind: "space" },
  { name: "space/16", cssVar: "--vz-space-16", kind: "space" },
  { name: "space/20", cssVar: "--vz-space-20", kind: "space" },
  { name: "space/24", cssVar: "--vz-space-24", kind: "space" },
  { name: "space/32", cssVar: "--vz-space-32", kind: "space" },
  { name: "space/48", cssVar: "--vz-space-48", kind: "space" },
];

export const toneModes = ["muted", "default", "faint", "inverse", "failed", "live", "retrying"] as const;
export type ToneMode = (typeof toneModes)[number];

/** The Color token each tone mode resolves to. */
export const toneAliases: Record<ToneMode, string> = {
  muted: "--vz-text-muted",
  default: "--vz-text",
  faint: "--vz-text-faint",
  inverse: "--vz-ground",
  failed: "--vz-failed",
  live: "--vz-live",
  retrying: "--vz-retrying",
};

export type TypeStyle = { name: string; utility: string; family: "sans" | "mono"; description: string };

export const typeStyles: TypeStyle[] = [
  { name: "Type/display", utility: "type-display", family: "sans", description: "Greetings and brand moments. One per screen" },
  { name: "Type/title", utility: "type-title", family: "sans", description: "Page titles" },
  { name: "Type/heading", utility: "type-heading", family: "sans", description: "Card, panel and section headings" },
  { name: "Type/prose", utility: "type-prose", family: "sans", description: "Chat replies and long reading" },
  { name: "Type/body", utility: "type-body", family: "sans", description: "Default text, nav items" },
  { name: "Type/body-para", utility: "type-body-para", family: "sans", description: "Multi-line body text" },
  { name: "Type/body-medium", utility: "type-body-medium", family: "sans", description: "Selected nav item, emphasised body" },
  { name: "Type/body-strong", utility: "type-body-strong", family: "sans", description: "Agent and item names" },
  { name: "Type/ui", utility: "type-ui", family: "sans", description: "Controls, inputs, table cells" },
  { name: "Type/ui-para", utility: "type-ui-para", family: "sans", description: "Descriptions and captions" },
  { name: "Type/ui-medium", utility: "type-ui-medium", family: "sans", description: "Buttons, tabs, emphasised controls" },
  { name: "Type/ui-strong", utility: "type-ui-strong", family: "sans", description: "Notice titles, primary buttons" },
  { name: "Type/small", utility: "type-small", family: "sans", description: "Metadata, helper text" },
  { name: "Type/small-para", utility: "type-small-para", family: "sans", description: "Multi-line helper text" },
  { name: "Type/small-medium", utility: "type-small-medium", family: "sans", description: "Small buttons, chips" },
  { name: "Type/small-strong", utility: "type-small-strong", family: "sans", description: "Small emphasis" },
  { name: "Type/label", utility: "type-label", family: "mono", description: "Uppercase section and column labels" },
  { name: "Type/mono-xs", utility: "type-mono-xs", family: "mono", description: "Timestamps, counts, ids" },
  { name: "Type/mono-xs-medium", utility: "type-mono-xs-medium", family: "mono", description: "Keys and card numbers" },
  { name: "Type/mono-sm", utility: "type-mono-sm", family: "mono", description: "Tool arguments, model ids, meta lines" },
  { name: "Type/mono-sm-medium", utility: "type-mono-sm-medium", family: "mono", description: "Tool and function names" },
  { name: "Type/code", utility: "type-code", family: "mono", description: "Inline code, keys, values" },
  { name: "Type/code-block", utility: "type-code-block", family: "mono", description: "Code blocks and diffs" },
  { name: "Type/code-medium", utility: "type-code-medium", family: "mono", description: "Skill and file names" },
];

export const elevations = [
  { name: "Elevation/popover", utility: "shadow-popover", description: "Menus, pickers, toasts" },
  { name: "Elevation/dialog", utility: "shadow-dialog", description: "Dialogs and confirmations" },
] as const;

/**
 * Motion presets for motion/react. Durations and easings mirror the CSS motion
 * tokens (--vz-duration-*, --vz-ease-* in tokens.css) — seconds there, seconds
 * here, one conceptual scale in two runtimes. Springs have no CSS analog, so
 * this is their only home.
 */
export const motionDurations = { fast: 0.12, base: 0.18, slow: 0.28 } as const;

export type MotionEase = [number, number, number, number];

export const motionEases = {
  /** The house ease: hover, press and other immediate feedback. */
  standard: [0.2, 0, 0, 1] as MotionEase,
  /** Deceleration for confident arrivals; exits run faster on the standard ease. */
  arrive: [0.16, 1, 0.3, 1] as MotionEase,
} as const;

export const motionSprings = {
  /** Switches, checkboxes, checks: a crisp settle with a breath of overshoot. */
  crisp: { type: "spring", stiffness: 600, damping: 45 },
  /** Sliding indicators (tabs, night strip): glides to rest. */
  glide: { type: "spring", stiffness: 420, damping: 36 },
} as const;
