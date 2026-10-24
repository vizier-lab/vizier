import { createContext, useContext, useId, type ButtonHTMLAttributes } from "react";
import { motion } from "motion/react";
import { cx } from "../../lib/cx";
import { motionSprings } from "../../tokens/tokens";

export type TabProps = {
  selected?: boolean;
  /** The red "Danger Zone" tab. */
  danger?: boolean;
} & ButtonHTMLAttributes<HTMLButtonElement>;

/** The tab strip's id, so the active underline glides from tab to tab. */
const TabGroupId = createContext<string | null>(null);

/** One tab (D8). Put tabs in a `TabList`; the panel is the caller's. */
export function Tab({ selected, danger, type = "button", className, children, ...rest }: TabProps) {
  const groupId = useContext(TabGroupId);
  return (
    <button
      type={type}
      role="tab"
      aria-selected={selected ?? false}
      className={cx(
        "group flex shrink-0 flex-col gap-10 rounded-xs pt-0 transition-colors disabled:opacity-40",
        selected ? "type-body-medium text-fg" : danger ? "type-body text-failed" : "type-body text-muted enabled:hover:text-soft",
        className,
      )}
      {...rest}
    >
      <span className="whitespace-nowrap">{children}</span>
      {selected && groupId ? (
        // The shared underline: it slides to the next tab rather than jumping.
        <motion.span aria-hidden layoutId={groupId} className="h-2 w-full bg-fg" transition={motionSprings.glide} />
      ) : (
        <span
          aria-hidden
          className={cx(
            "h-2 w-full transition-colors",
            selected ? "bg-fg" : "bg-transparent group-enabled:group-hover:bg-border-strong",
          )}
        />
      )}
    </button>
  );
}

export type TabListProps = { "aria-label": string } & React.HTMLAttributes<HTMLDivElement>;

/** A row of tabs on a hairline. */
export function TabList({ className, ...rest }: TabListProps) {
  const id = useId();
  return (
    <TabGroupId.Provider value={id}>
      <div role="tablist" className={cx("flex max-w-full gap-24 overflow-x-auto border-b border-border scrollbar-none", className)} {...rest} />
    </TabGroupId.Provider>
  );
}
