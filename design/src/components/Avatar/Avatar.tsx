import type { HTMLAttributes } from "react";
import BoringAvatar from "boring-avatars";
import { cx } from "../../lib/cx";

export type AvatarProps = {
  /** The person's name: seeds the generated face and gives the accessible name. */
  name: string;
  /** A photo; the generated face shows when absent. */
  src?: string;
  /** xs 22 (chat, as an agent's Sigil), sm 26 (lists), md 40 (menus), lg 56 (profile header). */
  size?: "xs" | "sm" | "md" | "lg";
  /** A rounded square like an agent's Sigil, instead of a circle. */
  square?: boolean;
};

const SIZES = {
  xs: { box: "size-22", px: 22, radius: 6 },
  sm: { box: "size-26", px: 26, radius: 7 },
  md: { box: "size-40", px: 40, radius: 10 },
  lg: { box: "size-56", px: 56, radius: 13 },
};

// Monochrome plus live, as hex (boring-avatars reads hex to pick the face colour, so no CSS variables):
// live, ink, text-soft, chart-4, chart-5, border. Light-theme values, used in both themes: the face is a
// picture, not a surface.
export const AVATAR_COLORS = ["#059669", "#111114", "#3f3f46", "#6b6b76", "#a1a1aa", "#e4e4e7"];

/** A person: a boring-avatars beam seeded by their name, or their photo (D40). Agents use Sigil. */
export function Avatar({ name, src, size = "md", square, className, style, ...rest }: AvatarProps & HTMLAttributes<HTMLSpanElement>) {
  const { box, px, radius } = SIZES[size];
  return (
    <span
      role="img"
      aria-label={name}
      className={cx(
        "relative inline-flex shrink-0 overflow-hidden bg-border select-none",
        square ? "border border-border-strong" : "rounded-pill",
        box,
        className,
      )}
      style={square ? { borderRadius: radius, ...style } : style}
      {...rest}
    >
      <span aria-hidden className="contents">
        <BoringAvatar name={name} variant="beam" size={px} colors={AVATAR_COLORS} square={square} />
      </span>
      {src && <img src={src} alt="" className="absolute inset-0 size-full object-cover" />}
    </span>
  );
}
