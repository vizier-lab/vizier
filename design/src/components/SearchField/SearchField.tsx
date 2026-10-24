import { useRef, type InputHTMLAttributes, type ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type SearchFieldProps = {
  value: string;
  onValueChange: (value: string) => void;
  /** Name for assistive tech; the placeholder carries the visible hint ("Search 48 agents"). */
  "aria-label": string;
  /** Right-aligned count: "48", or "38 of 1,024" while filtered. */
  count?: ReactNode;
} & Omit<InputHTMLAttributes<HTMLInputElement>, "value" | "onChange" | "type">;

/** Search over a list (D15). Clear shows once there is text; Esc also clears. */
export function SearchField({ value, onValueChange, count, className, disabled, onKeyDown, ...rest }: SearchFieldProps) {
  const ref = useRef<HTMLInputElement>(null);
  return (
    <div
      className={cx(
        "flex h-34 items-center gap-8 rounded-md border border-border-strong bg-surface px-12",
        "hover:border-border-hover focus-within:border-live hover:focus-within:border-live transition-colors",
        disabled && "pointer-events-none opacity-40",
        className,
      )}
    >
      <Icon name="search" size={14} tone="faint" />
      <input
        ref={ref}
        type="search"
        value={value}
        disabled={disabled}
        onChange={(e) => onValueChange(e.target.value)}
        onKeyDown={(e) => {
          onKeyDown?.(e);
          if (e.key === "Escape" && value) {
            e.preventDefault();
            onValueChange("");
          }
        }}
        className="h-full min-w-0 flex-1 bg-transparent type-ui text-fg outline-none placeholder:text-faint [&::-webkit-search-cancel-button]:hidden"
        {...rest}
      />
      {count != null && <span className="type-mono-xs text-faint whitespace-nowrap">{count}</span>}
      {value && (
        <button
          type="button"
          aria-label="Clear search"
          onClick={() => {
            onValueChange("");
            ref.current?.focus();
          }}
          className="flex size-18 items-center justify-center rounded-xs text-muted hover:text-fg"
        >
          <Icon name="close" size={13} />
        </button>
      )}
    </div>
  );
}
