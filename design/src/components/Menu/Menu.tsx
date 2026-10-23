import { useRef, type HTMLAttributes, type KeyboardEvent, type ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type MenuProps = {
  "aria-label": string;
  /** A filter field above the items; show it past about eight items. */
  search?: { value: string; onChange: (value: string) => void; placeholder: string };
  /** MenuItems and MenuDividers. */
  children: ReactNode;
} & HTMLAttributes<HTMLDivElement>;

const ITEM = '[role^="menuitem"]:not(:disabled)';

/**
 * A floating list of actions (D21). ↑ ↓ move between items, Home/End jump;
 * the caller positions it and closes it on Esc or an outside click.
 */
export function Menu({ search, className, children, onKeyDown, "aria-label": label, ...rest }: MenuProps) {
  const ref = useRef<HTMLDivElement>(null);
  const move = (e: KeyboardEvent<HTMLDivElement>) => {
    onKeyDown?.(e);
    const items = Array.from(ref.current?.querySelectorAll<HTMLElement>(ITEM) ?? []);
    if (!items.length) return;
    const i = items.indexOf(document.activeElement as HTMLElement);
    const go = (n: number) => {
      e.preventDefault();
      items[(n + items.length) % items.length].focus();
    };
    if (e.key === "ArrowDown") go(i + 1);
    else if (e.key === "ArrowUp") go(i < 0 ? items.length - 1 : i - 1);
    else if (e.key === "Home") go(0);
    else if (e.key === "End") go(items.length - 1);
  };
  return (
    <div
      ref={ref}
      onKeyDown={move}
      className={cx("flex w-252 flex-col rounded-lg border border-border-strong bg-surface p-6 shadow-popover", className)}
      {...rest}
    >
      {search && (
        <label className="mb-4 flex h-29 items-center gap-8 rounded-md bg-ground px-10 focus-within:outline focus-within:outline-live">
          <Icon name="search" size={13} tone="faint" />
          <input
            value={search.value}
            onChange={(e) => search.onChange(e.target.value)}
            placeholder={search.placeholder}
            aria-label={search.placeholder}
            className="min-w-0 flex-1 bg-transparent type-small text-fg outline-none placeholder:text-faint"
          />
        </label>
      )}
      <div role="menu" aria-label={label} className="flex flex-col">
        {children}
      </div>
    </div>
  );
}

/** A hairline between groups of items. */
export function MenuDivider() {
  return <div role="separator" className="my-4 h-px bg-border" />;
}
