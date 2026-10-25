import type { ButtonHTMLAttributes } from "react";
import { motion, type HTMLMotionProps } from "motion/react";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons/map";
import { cx } from "../../lib/cx";
import { motionSprings } from "../../tokens/tokens";

export type IconButtonProps = {
  icon: IconName;
  /** Required: an icon-only control needs a name. */
  "aria-label": string;
  variant?: "secondary" | "ghost";
  /** md 34 (toolbars), sm 26 (close × in panels). */
  size?: "sm" | "md";
  /** A toggle that is on. Sets aria-pressed. */
  selected?: boolean;
  /** A count badge in the corner, e.g. unread notifications. Hidden at zero, capped at 99+. */
  count?: number;
  /** A separate live dot, e.g. an agent setting up. Not part of the count. */
  busy?: boolean;
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
  count,
  busy,
  type = "button",
  className,
  ...rest
}: IconButtonProps) {
  const v = VARIANTS[variant];
  const unread = count != null && count > 0 ? count : 0;
  return (
    <motion.button
      type={type}
      aria-pressed={selected}
      whileTap={{ scale: 0.94 }}
      transition={motionSprings.crisp}
      className={cx(
        "relative inline-flex shrink-0 items-center justify-center transition-colors disabled:opacity-40",
        SIZES[size].box,
        v.base,
        selected && v.selected,
        className,
      )}
      {...(rest as HTMLMotionProps<"button">)}
    >
      <Icon name={icon} size={SIZES[size].icon} />
      {/* Visual only: the caller's aria-label carries the count and busy state in words. */}
      {unread > 0 && (
        <span
          aria-hidden
          className="absolute -top-3 -right-3 flex h-17 min-w-17 items-center justify-center rounded-pill bg-live px-4 type-mono-xs text-ground"
        >
          {unread > 99 ? "99+" : unread}
        </span>
      )}
      {busy && (
        <span
          aria-hidden
          className="absolute -right-1 -bottom-1 size-7 rounded-pill bg-live animate-dot-wave motion-reduce:animate-none"
        />
      )}
    </motion.button>
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
