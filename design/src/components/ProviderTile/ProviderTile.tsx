import type { ButtonHTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { ProviderLogo } from "../ProviderLogo/ProviderLogo";

export type ProviderTileProps = {
  provider: string;
  name: string;
  logo?: { src?: string | null; fullColor?: boolean };
  /** "Gemini", "300+ models", "sign in with GitHub". */
  sub?: ReactNode;
} & ButtonHTMLAttributes<HTMLButtonElement>;

/** A provider to add, in "Add a provider" (X65). */
export function ProviderTile({ provider, name, logo, sub, type = "button", className, ...rest }: ProviderTileProps) {
  return (
    <button
      type={type}
      className={cx("flex items-center gap-10 rounded-lg border border-border-strong bg-ground px-12 py-8 text-left hover:bg-raised active:bg-pressed", className)}
      {...rest}
    >
      <ProviderLogo provider={provider} name={name} src={logo?.src} fullColor={logo?.fullColor} />
      <span className="flex min-w-0 flex-col gap-2">
        <span className="truncate type-ui-medium text-fg">{name}</span>
        {sub && <span className="truncate type-small text-faint">{sub}</span>}
      </span>
    </button>
  );
}
