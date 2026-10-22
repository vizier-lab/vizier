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
} & HTMLAttributes<HTMLSpanElement>;

function initials(name: string) {
  const words = name.trim().split(/\s+/);
  return (words.length > 1 ? words[0][0] : name.slice(0, 2)).replace(/^./, (c) => c.toUpperCase()).slice(0, 2);
}

/** A provider's letter tile (X63). Known providers in their brand colour; others neutral. */
export function ProviderLogo({ provider, name, size = "md", className, style, ...rest }: ProviderLogoProps) {
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
