import type { SVGProps } from "react";
import { cx } from "../../lib/cx";
import { integrationBrands } from "./brands";

export type IntegrationLogoProps = {
  /** Integration kind id: "discord", "telegram", "brave_search", "office", "shell", "http", "mcp". */
  kind: string;
  /** Box size in px: 28 in lists, 44 on cards. */
  size?: number;
} & Omit<SVGProps<SVGSVGElement>, "children">;

/** An integration's logo tile (X60). Unknown kinds fall back to the MCP plug. */
export function IntegrationLogo({ kind, size = 44, className, ...rest }: IntegrationLogoProps) {
  const brand = integrationBrands[kind] ?? integrationBrands.mcp;
  return (
    <svg
      viewBox="0 0 64 64"
      width={size}
      height={size}
      role="img"
      aria-label={brand.name}
      className={cx("shrink-0", className)}
      {...rest}
    >
      <rect width="64" height="64" rx="14" fill={brand.bg} />
      {/* Static, trusted markup from brands.ts. */}
      <g dangerouslySetInnerHTML={{ __html: brand.glyph }} />
    </svg>
  );
}
