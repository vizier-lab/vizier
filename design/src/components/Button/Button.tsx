import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons/map";
import { cx } from "../../lib/cx";
import { DotLoader } from "../DotLoader/DotLoader";
import { Kbd } from "../Kbd/Kbd";

export type ButtonVariant = "primary" | "secondary" | "danger" | "destructive" | "ghost" | "dashed";
export type ButtonSize = "sm" | "md" | "lg";

export type ButtonProps = {
  variant?: ButtonVariant;
  /** sm 28, md 34, lg 48 (sign-in and sign-up, usually with `block`). */
  size?: ButtonSize;
  /** Leading icon; replaced by the dot loader while loading. */
  icon?: IconName;
  /** Trailing icon, e.g. `chevron-right` on "Continue to review". */
  trailingIcon?: IconName;
  /** Key hint after the label, e.g. "Ctrl S". */
  shortcut?: string;
  /** Shows the dot loader and blocks clicks; pass the in-progress label ("Saving…") as children. */
  loading?: boolean;
  /** Full width. */
  block?: boolean;
  children?: ReactNode;
} & ButtonHTMLAttributes<HTMLButtonElement>;

// Interaction states are CSS: hover → raised/ink-hover, pressed → pressed/ink-pressed, disabled → 40%.
const VARIANTS: Record<ButtonVariant, string> = {
  primary: "bg-ink text-ground enabled:hover:bg-ink-hover enabled:active:bg-ink-pressed",
  secondary: "border border-border-strong text-fg enabled:hover:bg-raised enabled:active:bg-pressed",
  danger:
    "border border-failed-line text-failed enabled:hover:bg-failed-bg enabled:active:border-failed enabled:active:bg-failed-bg",
  destructive:
    "border border-failed-line bg-failed text-ground enabled:hover:bg-failed-hover enabled:active:bg-failed-pressed",
  ghost: "text-muted enabled:hover:bg-raised enabled:hover:text-fg enabled:active:bg-pressed enabled:active:text-fg",
  dashed:
    "border border-dashed border-border-strong text-fg enabled:hover:border-border-hover enabled:hover:bg-surface enabled:active:bg-pressed",
};

// Strong labels on filled buttons, medium on outlined ones.
const STRONG: Record<ButtonVariant, boolean> = {
  primary: true,
  destructive: true,
  secondary: false,
  danger: false,
  ghost: false,
  dashed: false,
};

const SIZES: Record<ButtonSize, { box: string; withIcon: string; icon: number; strong: string; medium: string }> = {
  sm: { box: "h-28 gap-6 rounded-sm px-10", withIcon: "pl-8", icon: 13, strong: "type-small-strong", medium: "type-small-medium" },
  md: { box: "h-34 gap-6 rounded-md px-14", withIcon: "pl-12", icon: 15, strong: "type-ui-strong", medium: "type-ui-medium" },
  lg: { box: "h-48 gap-8 rounded-md px-20", withIcon: "pl-16", icon: 17, strong: "type-heading", medium: "type-heading" },
};

/** The button (D10). */
export function Button({
  variant = "secondary",
  size = "md",
  icon,
  trailingIcon,
  shortcut,
  loading = false,
  block = false,
  disabled,
  type = "button",
  className,
  children,
  ...rest
}: ButtonProps) {
  const s = SIZES[size];
  const leading = loading || icon;
  return (
    <button
      type={type}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      className={cx(
        "inline-flex shrink-0 items-center justify-center whitespace-nowrap select-none disabled:opacity-40",
        loading && "disabled:opacity-100",
        s.box,
        leading && s.withIcon,
        STRONG[variant] ? s.strong : s.medium,
        VARIANTS[variant],
        block && "w-full",
        className,
      )}
      {...rest}
    >
      {loading ? <DotLoader /> : icon ? <Icon name={icon} size={s.icon} /> : null}
      {children}
      {shortcut && <Kbd look="inside">{shortcut}</Kbd>}
      {trailingIcon && <Icon name={trailingIcon} size={s.icon} />}
    </button>
  );
}
