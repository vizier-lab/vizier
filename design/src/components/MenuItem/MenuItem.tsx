import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons/map";
import { cx } from "../../lib/cx";

export type MenuItemProps = {
  icon?: IconName;
  label: ReactNode;
  /** Right-aligned hint, usually a shortcut ("F2"). */
  meta?: ReactNode;
  tone?: "default" | "danger";
  /** The current choice in a select-like menu: medium weight and a ✓. */
  selected?: boolean;
  /** Keyboard highlight in a menu driven by aria-activedescendant; looks like hover. */
  highlighted?: boolean;
} & ButtonHTMLAttributes<HTMLButtonElement>;

/** One row of a Menu (D22). Hover is also the keyboard highlight. */
export function MenuItem({
  icon,
  label,
  meta,
  tone = "default",
  selected,
  highlighted,
  type = "button",
  role = "menuitem",
  className,
  ...rest
}: MenuItemProps) {
  const danger = tone === "danger";
  return (
    <button
      type={type}
      role={role}
      aria-checked={role === "menuitemradio" || role === "menuitemcheckbox" ? (selected ?? false) : undefined}
      aria-selected={role === "option" ? (selected ?? false) : undefined}
      className={cx(
        "group flex h-30 w-full items-center gap-10 rounded-sm px-8 text-left disabled:opacity-40",
        danger ? "text-failed" : selected ? "text-fg type-ui-medium" : "text-soft",
        !selected && "type-ui",
        danger ? "enabled:hover:bg-failed-bg" : "enabled:hover:bg-raised enabled:hover:text-fg",
        "enabled:active:bg-pressed",
        highlighted && (danger ? "bg-failed-bg" : "bg-raised text-fg"),
        className,
      )}
      {...rest}
    >
      {icon && (
        <Icon
          name={icon}
          size={15}
          className={danger ? "" : selected || highlighted ? "text-fg" : "text-muted group-hover:text-fg"}
        />
      )}
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {selected ? <Icon name="check" size={13} className="text-fg" /> : meta && <span className="type-mono-xs text-faint">{meta}</span>}
    </button>
  );
}
