import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";
import { providerBrands, tileLetterSize } from "./brands";

export type ProviderLogoProps = {
  /** Provider id: "anthropic", "openai", "ollama", a custom provider's id… */
  provider: string;
  /** The provider's display name, for the accessible name and the fallback letters. */
  name: string;
  /** sm 18 (inside selects), md 34 (rows and tiles). */
  size?: "sm" | "md";
  /** The catalogue's logo (an SVG url). Without one, the letter tile stands in. */
  src?: string | null;
  /** The logo has its own colours; otherwise it is drawn in the text colour, so it follows the theme. */
  fullColor?: boolean;
} & HTMLAttributes<HTMLSpanElement>;

function initials(name: string) {
  const words = name.trim().split(/\s+/);
  return (words.length > 1 ? words[0][0] : name.slice(0, 2)).replace(/^./, (c) => c.toUpperCase()).slice(0, 2);
}

const glyph = { sm: 14, md: 20 } as const;

/**
 * A provider's logo (X63). With the catalogue's SVG it is the real mark on a neutral tile (sm: bare);
 * single-colour marks are masks filled with the text colour, so they read in both themes. Without
 * one, a letter tile: known providers in their brand colour, others neutral.
 */
export function ProviderLogo({ provider, name, size = "md", src, fullColor, className, style, ...rest }: ProviderLogoProps) {
  if (src) {
    const g = glyph[size];
    return (
      <span
        role="img"
        aria-label={name}
        className={cx(
          "inline-flex shrink-0 items-center justify-center select-none",
          size === "sm" ? "size-18" : "size-34 rounded-md border border-border bg-unlit",
          className,
        )}
        style={style}
        {...rest}
      >
        {fullColor ? (
          <img src={src} alt="" width={g} height={g} draggable={false} style={{ width: g, height: g }} />
        ) : (
          <span
            aria-hidden
            className="bg-fg"
            style={{
              width: g,
              height: g,
              maskImage: `url(${src})`,
              WebkitMaskImage: `url(${src})`,
              maskSize: "contain",
              WebkitMaskSize: "contain",
              maskRepeat: "no-repeat",
              WebkitMaskRepeat: "no-repeat",
              maskPosition: "center",
              WebkitMaskPosition: "center",
            }}
          />
        )}
      </span>
    );
  }
  const brand = providerBrands[provider];
  const letters = brand?.letters ?? (size === "sm" ? name.charAt(0).toUpperCase() : initials(name));
  return (
    <span
      role="img"
      aria-label={name}
      className={cx(
        "inline-flex shrink-0 items-center justify-center font-sans font-bold select-none",
        size === "sm" ? "size-18 rounded-xs" : "size-34 rounded-md",
        !brand && "bg-unlit text-fg",
        className,
      )}
      style={{ fontSize: tileLetterSize[size], ...(brand && { background: brand.bg, color: brand.fg }), ...style }}
      {...rest}
    >
      <span aria-hidden>{letters}</span>
    </span>
  );
}
