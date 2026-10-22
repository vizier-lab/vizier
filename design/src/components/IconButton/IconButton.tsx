import type { ButtonHTMLAttributes } from "react";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons/map";
import { cx } from "../../lib/cx";

export type IconButtonProps = {
  icon: IconName;
  /** Required: an icon-only control needs a name. */
  "aria-label": string;
  variant?: "secondary" | "ghost";
  /** md 34 (toolbars), sm 26 (close × in panels). */
  size?: "sm" | "md";
  /** A toggle that is on. Sets aria-pressed. */
  selected?: boolean;
} & Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children">;

const VARIANTS = {
  secondary: {
    base: "border border-border-strong text-fg enabled:hover:bg-raised enabled:active:bg-pressed",
    selected: "border-border-hover bg-raised",
  },
  ghost: {
    base: "text-muted enabled:hover:bg-raised enabled:hover:text-fg enabled:active:bg-pressed enabled:active:text-fg",
    selected: "bg-raised text-fg",
  },
};

const SIZES = { sm: { box: "size-26 rounded-sm", icon: 13 }, md: { box: "size-34 rounded-md", icon: 15 } };

/** An icon-only button (D11). Group them with `IconButtonGroup`. */
export function IconButton({
  icon,
  variant = "secondary",
  size = "md",
  selected,
  type = "button",
  className,
  ...rest
}: IconButtonProps) {
  const v = VARIANTS[variant];
  return (
    <button
      type={type}
      aria-pressed={selected}
      className={cx(
        "inline-flex shrink-0 items-center justify-center disabled:opacity-40",
        SIZES[size].box,
        v.base,
        selected && v.selected,
        className,
      )}
      {...rest}
    >
      <Icon name={icon} size={SIZES[size].icon} />
    </button>
  );
}

export type IconButtonGroupProps = {
  "aria-label": string;
  orientation?: "vertical" | "horizontal";
  children: React.ReactNode;
} & React.HTMLAttributes<HTMLDivElement>;

/** Joined icon buttons, e.g. the memory graph's zoom in / zoom out / fit. */
export function IconButtonGroup({ orientation = "vertical", className, ...rest }: IconButtonGroupProps) {
  return (
    <div
      role="group"
      className={cx(
        "inline-flex overflow-hidden rounded-md border border-border-strong bg-surface",
        orientation === "vertical" ? "flex-col divide-y divide-border" : "divide-x divide-border",
        "[&>button]:rounded-none [&>button]:border-0",
        className,
      )}
      {...rest}
    />
  );
}
