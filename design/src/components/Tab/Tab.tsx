import type { ButtonHTMLAttributes } from "react";
import { cx } from "../../lib/cx";

export type TabProps = {
  selected?: boolean;
  /** The red "Danger Zone" tab. */
  danger?: boolean;
} & ButtonHTMLAttributes<HTMLButtonElement>;

/** One tab (D8). Put tabs in a `TabList`; the panel is the caller's. */
export function Tab({ selected, danger, type = "button", className, children, ...rest }: TabProps) {
  return (
    <button
      type={type}
      role="tab"
      aria-selected={selected ?? false}
      className={cx(
        "group flex shrink-0 flex-col gap-10 rounded-xs pt-0 disabled:opacity-40",
        selected ? "type-body-medium text-fg" : danger ? "type-body text-failed" : "type-body text-muted enabled:hover:text-soft",
        className,
      )}
      {...rest}
    >
      <span className="whitespace-nowrap">{children}</span>
      <span
        aria-hidden
        className={cx(
          "h-2 w-full",
          selected ? "bg-fg" : "bg-transparent group-enabled:group-hover:bg-border-strong",
        )}
      />
    </button>
  );
}

export type TabListProps = { "aria-label": string } & React.HTMLAttributes<HTMLDivElement>;

/** A row of tabs on a hairline. */
export function TabList({ className, ...rest }: TabListProps) {
  return <div role="tablist" className={cx("flex gap-24 border-b border-border", className)} {...rest} />;
}
