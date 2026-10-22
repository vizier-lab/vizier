import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";

export type AvatarProps = {
  /** The person's name: gives the initial and the accessible name. */
  name: string;
  /** A photo; the initial shows when absent or while loading. */
  src?: string;
  /** sm 26 (lists), md 40 (menus), lg 56 (profile header). */
  size?: "sm" | "md" | "lg";
} & HTMLAttributes<HTMLSpanElement>;

const SIZES = {
  sm: "size-26 type-small-strong",
  md: "size-40 type-body-strong",
  lg: "size-56 type-title",
};

/** A person: their initial on a round chip, or their photo (D40). Agents use Sigil. */
export function Avatar({ name, src, size = "md", className, ...rest }: AvatarProps) {
  const initial = name.trim().charAt(0).toUpperCase() || "?";
  return (
    <span
      role="img"
      aria-label={name}
      className={cx(
        "relative inline-flex shrink-0 items-center justify-center overflow-hidden rounded-pill bg-border text-fg select-none",
        SIZES[size],
        className,
      )}
      {...rest}
    >
      <span aria-hidden>{initial}</span>
      {src && <img src={src} alt="" className="absolute inset-0 size-full object-cover" />}
    </span>
  );
}
