import type { CSSProperties, HTMLAttributes } from "react";
import { colorTokens } from "../../tokens/tokens";

/** What Tailwind calls each token's variable, so a utility inside the scope follows it too. */
const TAILWIND_VAR = new Map(colorTokens.map((t) => [t.cssVar, `--color-${t.tw}`]));

export type ThemeScopeProps = {
  /** Token variables to their colours, e.g. `{ "--vz-panel": "#ffffff" }`. */
  vars: Record<string, string>;
  /** The appearance the colours are for, so native controls and scrollbars match. */
  appearance?: "light" | "dark";
} & HTMLAttributes<HTMLDivElement>;

/**
 * Draws its children in another scheme's colours, whatever the page is showing: real components, not
 * a picture of them. Tailwind's `--color-*` are resolved where they are declared, on `:root`, and
 * inherited as finished values, so each token is set under both names.
 */
export function ThemeScope({ vars, appearance, style, children, ...rest }: ThemeScopeProps) {
  const scoped: Record<string, string> = {};
  for (const [name, value] of Object.entries(vars)) {
    scoped[name] = value;
    const tailwind = TAILWIND_VAR.get(name);
    if (tailwind) scoped[tailwind] = value;
  }
  return (
    <div style={{ ...scoped, color: "var(--vz-text)", colorScheme: appearance, ...style } as CSSProperties} {...rest}>
      {children}
    </div>
  );
}
