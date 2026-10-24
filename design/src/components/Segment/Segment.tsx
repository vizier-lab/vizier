import { createContext, useContext, type ButtonHTMLAttributes, type ReactNode } from "react";
import { motion } from "motion/react";
import { cx } from "../../lib/cx";
import { motionSprings } from "../../tokens/tokens";

export type SegmentProps = {
  selected?: boolean;
  /** Mono count after the label. */
  count?: ReactNode;
} & ButtonHTMLAttributes<HTMLButtonElement>;

/** The strip's id, so the selected pill slides from option to option. */
export const SegmentGroupId = createContext<string | null>(null);

/** One option of FilterTabs (D16). */
export function Segment({ selected, count, type = "button", className, children, ...rest }: SegmentProps) {
  const groupId = useContext(SegmentGroupId);
  const moving = selected && groupId !== null;
  return (
    <button
      type={type}
      aria-pressed={selected ?? false}
      className={cx(
        // isolate keeps the sliding pill behind the label without letting it
        // fall behind the strip's own ground.
        "relative isolate inline-flex h-25 shrink-0 items-center gap-4 rounded-sm px-8 type-small-medium whitespace-nowrap transition-colors disabled:opacity-40",
        moving ? "text-fg" : selected ? "bg-raised text-fg" : "text-muted enabled:hover:text-fg enabled:active:bg-pressed",
        className,
      )}
      {...rest}
    >
      {moving && <motion.span aria-hidden layoutId={groupId} className="absolute inset-0 -z-10 rounded-sm bg-raised" transition={motionSprings.glide} />}
      <span className="relative">{children}</span>
      {count != null && <span className={cx("relative type-mono-xs", selected ? "text-muted" : "text-faint")}>{count}</span>}
    </button>
  );
}
