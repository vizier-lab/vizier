import { useEffect, useRef, type KeyboardEvent, type ReactNode } from "react";
import { cx } from "../../lib/cx";
import { MenuItem } from "../MenuItem/MenuItem";

export type ListboxOption<V extends string> = { value: V; label: ReactNode; leading?: ReactNode };

/**
 * The popup list shared by Select and SortSelect. Focus moves to the chosen
 * option when it opens; ↑ ↓ Home End move, Enter picks, Esc and Tab close.
 */
export function ListboxPopup<V extends string>({
  id,
  label,
  options,
  value,
  onPick,
  onClose,
  align = "start",
  className,
}: {
  id: string;
  label: string;
  options: ListboxOption<V>[];
  value?: V;
  onPick: (value: V) => void;
  onClose: () => void;
  align?: "start" | "end";
  className?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const items = ref.current?.querySelectorAll<HTMLElement>('[role="option"]');
    const chosen = ref.current?.querySelector<HTMLElement>('[aria-selected="true"]');
    (chosen ?? items?.[0])?.focus();
  }, []);
  const onKeyDown = (e: KeyboardEvent) => {
    const items = Array.from(ref.current?.querySelectorAll<HTMLElement>('[role="option"]') ?? []);
    const i = items.indexOf(document.activeElement as HTMLElement);
    const go = (n: number) => {
      e.preventDefault();
      items[(n + items.length) % items.length]?.focus();
    };
    if (e.key === "ArrowDown") go(i + 1);
    else if (e.key === "ArrowUp") go(i - 1);
    else if (e.key === "Home") go(0);
    else if (e.key === "End") go(items.length - 1);
    else if (e.key === "Escape" || e.key === "Tab") {
      if (e.key === "Escape") e.preventDefault();
      onClose();
    }
  };
  return (
    <div
      ref={ref}
      id={id}
      role="listbox"
      aria-label={label}
      onKeyDown={onKeyDown}
      className={cx(
        "absolute top-full z-30 mt-4 flex min-w-full flex-col rounded-lg border border-border-strong bg-surface p-6 shadow-popover",
        align === "end" ? "right-0" : "left-0",
        className,
      )}
    >
      {options.map((o) => (
        <MenuItem
          key={o.value}
          role="option"
          selected={o.value === value}
          label={
            <span className="flex items-center gap-8">
              {o.leading}
              {o.label}
            </span>
          }
          onClick={() => onPick(o.value)}
          className="whitespace-nowrap"
        />
      ))}
    </div>
  );
}

/** Closes on a pointer-down outside `root`. */
export function useOutsideClose(open: boolean, root: React.RefObject<HTMLElement | null>, close: () => void) {
  useEffect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      if (!root.current?.contains(e.target as Node)) close();
    };
    document.addEventListener("pointerdown", onDown);
    return () => document.removeEventListener("pointerdown", onDown);
  }, [open, root, close]);
}
