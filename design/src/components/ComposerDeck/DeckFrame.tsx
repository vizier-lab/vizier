import type { HTMLAttributes, ReactNode } from "react";
import { motion, useReducedMotion } from "motion/react";
import { cx } from "../../lib/cx";
import { motionDurations, motionEases } from "../../tokens/tokens";

export type DeckFrameProps = {
  /** The header's label: "Commands and skills", "Files". */
  label: ReactNode;
  /** A muted line at the foot of the card: the keys. */
  footer?: ReactNode;
  children: ReactNode;
  /** Placement (where the deck stands); goes on the frame that moves. */
  className?: string;
} & Omit<HTMLAttributes<HTMLDivElement>, "children" | "className">;

/**
 * The composer deck's look for a popup that stands on the composer (the slash and "@" menus):
 * a frosted frame in the deck's colour whose 36px header carries the label, and the content in
 * a solid card in the composer's colour, inset 6px so a glass rim frames it. Place it where the
 * deck stands (inset 24px, on the composer's top edge) so it opens like a deck sheet.
 *
 * Motion, as the deck's sheet: it rises out of the composer to its height on the arrive ease and
 * folds back faster on the standard ease. Mount it inside `Presence` so it is seen leaving. With
 * reduced motion only the fade remains.
 */
export function DeckFrame({ label, footer, children, className, ...rest }: DeckFrameProps) {
  const reduced = useReducedMotion();
  return (
    <motion.div
      initial={{ height: reduced ? "auto" : 0, opacity: 0 }}
      animate={{ height: "auto", opacity: 1, transition: { duration: motionDurations.slow, ease: motionEases.arrive } }}
      exit={{ height: reduced ? "auto" : 0, opacity: 0, transition: { duration: motionDurations.base, ease: motionEases.standard } }}
      className={cx(
        "overflow-hidden rounded-t-xl border border-b-0 border-border-strong/60 bg-deck/60 shadow-popover backdrop-blur-lg backdrop-saturate-150",
        className,
      )}
    >
      <div className="flex flex-col" {...rest}>
        <span className="flex h-36 shrink-0 items-center px-12 type-label uppercase text-fg">{label}</span>
        <div className="mx-6 flex min-h-0 flex-col rounded-t-lg border border-b-0 border-border bg-surface p-6">
          {children}
          {footer && <p className="px-8 pt-6 pb-2 type-mono-xs text-faint">{footer}</p>}
        </div>
      </div>
    </motion.div>
  );
}
