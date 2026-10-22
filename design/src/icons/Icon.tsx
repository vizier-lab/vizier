import type { SVGProps } from "react";
import type { ToneMode } from "../tokens/tokens";
import { cx } from "../lib/cx";
import { icons, type IconName } from "./map";

export type IconProps = {
  name: IconName;
  /** Colour from the tone scale; inherits the text colour when unset. */
  tone?: ToneMode;
  /** Box size in px. The stroke stays 1.7px at any size. */
  size?: number;
  /** Makes the icon labelled; without it the icon is decorative. */
  title?: string;
} & Omit<SVGProps<SVGSVGElement>, "name">;

export function Icon({ name, tone, size = 17, title, className, ref, ...rest }: IconProps) {
  const Glyph = icons[name];
  const labelled = title !== undefined;
  return (
    <Glyph
      ref={ref}
      size={size}
      strokeWidth={1.7}
      absoluteStrokeWidth
      aria-hidden={labelled ? undefined : true}
      aria-label={labelled ? title : undefined}
      role={labelled ? "img" : undefined}
      focusable="false"
      className={cx("shrink-0", tone && `tone-${tone}`, className)}
      {...rest}
    />
  );
}
