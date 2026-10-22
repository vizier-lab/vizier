import { useOf } from "@storybook/addon-docs/blocks";

export type StoryMeta = {
  /** Inventory id: "D10", "X41", "B3". */
  id: string;
  /** The WebUI file this entry will replace, if any. */
  replaces?: string | null;
  /** Inventory ids folded into this entry. */
  merges?: string[];
};

/**
 * Top of every component docs page: where the entry comes from in the plan
 * (inventory id), what it will replace in the WebUI, and what it merges.
 * Deliberately no FigCraft reference: the catalogue is the source of truth.
 */
export function SourceBlock() {
  const resolved = useOf("meta", ["meta"]);
  const meta = resolved.preparedMeta;
  const vizier = meta.parameters?.vizier as StoryMeta | undefined;
  if (!vizier) return null;
  const review = meta.tags?.includes("approved") ? "approved" : "needs-review";

  const row = (label: string, value: React.ReactNode) => (
    <div style={{ display: "contents" }}>
      <dt style={{ opacity: 0.6 }}>{label}</dt>
      <dd style={{ margin: 0 }}>{value}</dd>
    </div>
  );

  return (
    <dl
      style={{
        display: "grid",
        gridTemplateColumns: "max-content 1fr",
        gap: "4px 16px",
        margin: "0 0 24px",
        padding: "12px 16px",
        border: "1px solid rgba(128,128,128,0.3)",
        borderRadius: 8,
        fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
        fontSize: 12.5,
      }}
    >
      {row("inventory", vizier.id)}
      {row("replaces", vizier.replaces ?? "—")}
      {vizier.merges?.length ? row("merges", vizier.merges.join(", ")) : null}
      {row("review", review)}
    </dl>
  );
}
