import { createContext, useContext, type ButtonHTMLAttributes, type ReactNode } from "react";
import { motion } from "motion/react";
import { cx } from "../../lib/cx";

export type SegmentProps = {
  selected?: boolean;
  /** Mono count after the label. */
  count?: ReactNode;
  /** `live` paints the selected option emerald, `solid` inverts it (foreground fill); e.g. Build / Plan in a mode switch. */
  tone?: "live" | "solid";
} & ButtonHTMLAttributes<HTMLButtonElement>;

/** The strip's id, so the selected pill slides from option to option. */
export const SegmentGroupId = createContext<string | null>(null);

/** One option of FilterTabs (D16). */
export function Segment({ selected, count, tone, type = "button", className, children, ...rest }: SegmentProps) {
  const groupId = useContext(SegmentGroupId);
  const moving = selected && groupId !== null;
  const live = selected && tone === "live";
  const solid = selected && tone === "solid";
  return (
    <button
      type={type}
      aria-pressed={selected ?? false}
      className={cx(
        // isolate keeps the sliding pill behind the label without letting it
        // fall behind the strip's own ground.
        "relative isolate inline-flex h-25 shrink-0 items-center gap-4 rounded-sm px-8 type-small-medium whitespace-nowrap transition-colors disabled:opacity-40",
        live ? (moving ? "text-white" : "bg-live text-white") : solid ? (moving ? "text-ground" : "bg-ink text-ground") : moving ? "text-fg" : selected ? "bg-raised text-fg" : "text-muted enabled:hover:text-fg enabled:active:bg-pressed",
        className,
      )}
      {...rest}
    >
      {moving && <motion.span aria-hidden layoutId={groupId} className="absolute inset-0 -z-10 rounded-sm" initial={{ backgroundColor: "var(--vz-raised)" }} animate={{ backgroundColor: live ? "var(--vz-live)" : solid ? "var(--vz-ink)" : "var(--vz-raised)" }} transition={{ type: "spring", stiffness: 520, damping: 24, mass: 0.9 }} />}
      <span className="relative">{children}</span>
      {count != null && <span className={cx("relative type-mono-xs", selected ? "text-muted" : "text-faint")}>{count}</span>}
    </button>
  );
}
