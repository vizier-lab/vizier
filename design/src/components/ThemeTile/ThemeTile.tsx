import type { ButtonHTMLAttributes } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type ThemeTileColors = {
  /** The app frame around the inset. */
  frame: string;
  /** The inset. */
  content: string;
  /** The edge of the miniature. */
  line: string;
  /** The miniature's lines of code, one colour each. */
  code: [string, string, string, string];
};

export type ThemeTileProps = {
  name: string;
  /** "Light and dark", "Dark only", "Custom". */
  detail: string;
  /** The scheme's own colours, so the tile shows the scheme and not the page. */
  colors: ThemeTileColors;
  selected?: boolean;
} & Omit<ButtonHTMLAttributes<HTMLButtonElement>, "name">;

const LINE_WIDTHS = ["60%", "80%", "45%", "30%"];

/** One colour scheme to pick: a miniature in its own colours, its name and what it offers. */
export function ThemeTile({ name, detail, colors, selected, className, type = "button", ...rest }: ThemeTileProps) {
  return (
    <button
      type={type}
      aria-pressed={selected ?? false}
      className={cx(
        "flex min-w-0 flex-col gap-6 rounded-lg border bg-panel p-6 text-left transition-colors",
        selected ? "border-live" : "border-border hover:border-border-hover",
        className,
      )}
      // A second pixel of edge, drawn so the tile doesn't shift when it is selected.
      style={selected ? { boxShadow: "0 0 0 1px var(--vz-live)" } : undefined}
      {...rest}
    >
      <span aria-hidden className="flex h-50 overflow-hidden rounded-sm border" style={{ background: colors.frame, borderColor: colors.line }}>
        <span className="w-[26%] shrink-0" />
        <span className="my-4 mr-4 flex min-w-0 flex-1 flex-col gap-2 rounded-xs p-4" style={{ background: colors.content }}>
          {colors.code.map((color, i) => (
            <span key={i} className="h-3 shrink-0" style={{ background: color, width: LINE_WIDTHS[i] }} />
          ))}
        </span>
      </span>
      <span className="flex min-w-0 flex-col gap-2">
        <span className="flex min-w-0 items-center justify-between gap-4 type-small-medium text-fg">
          <span className="truncate">{name}</span>
          {selected && <Icon name="check" size={13} className="shrink-0 text-live-text" />}
        </span>
        <span className="truncate type-small text-faint">{detail}</span>
      </span>
    </button>
  );
}
