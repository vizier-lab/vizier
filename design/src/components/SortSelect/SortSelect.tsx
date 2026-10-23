import { useCallback, useId, useRef, useState } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { ListboxPopup, useOutsideClose } from "../Select/Listbox";

export type SortSelectProps = {
  /** "Sort", "Break down by", "Sorted by". */
  label: string;
  value: string;
  options: { value: string; label: string }[];
  onChange: (value: string) => void;
  /** Bordered in toolbars; text inside panels. */
  look?: "bordered" | "text";
  className?: string;
};

/** How a list is ordered or broken down (X12): one component, two looks. */
export function SortSelect({ label, value, options, onChange, look = "bordered", className }: SortSelectProps) {
  const id = useId();
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const close = useCallback(() => setOpen(false), []);
  useOutsideClose(open, root, close);
  const current = options.find((o) => o.value === value);
  return (
    <div ref={root} className={cx("relative inline-flex", className)}>
      <button
        ref={button}
        type="button"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? `${id}-list` : undefined}
        onClick={() => setOpen(!open)}
        className={cx(
          "inline-flex items-center gap-6 whitespace-nowrap",
          look === "bordered"
            ? "h-34 rounded-md border border-border-strong px-12 hover:bg-raised"
            : "rounded-xs hover:underline",
        )}
      >
        <span className={look === "bordered" ? "type-ui text-faint" : "type-small text-faint"}>{label}</span>
        <span className={look === "bordered" ? "type-ui-medium text-fg" : "type-small text-muted"}>{current?.label}</span>
        <Icon name="chevron-down" size={13} tone={look === "bordered" ? "muted" : "faint"} />
      </button>
      {open && (
        <ListboxPopup
          id={`${id}-list`}
          label={label}
          options={options}
          value={value}
          align="end"
          onPick={(v) => {
            onChange(v);
            setOpen(false);
            button.current?.focus();
          }}
          onClose={() => {
            setOpen(false);
            button.current?.focus();
          }}
        />
      )}
    </div>
  );
}
