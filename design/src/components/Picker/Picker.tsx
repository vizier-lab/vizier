import {
  cloneElement,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactElement,
  type ReactNode,
} from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { Floating, inFloating } from "../Floating/Floating";

export type PickerSection<T> = { label?: ReactNode; items: T[] };

export type PickerProps<T> = {
  /** Names the list for assistive tech: "Agents", "Conversations". */
  label: string;
  /** The button that opens it (AgentSwitcher, a topic title, a bundle button). Omit to show the list inline. */
  trigger?: ReactElement<{ onClick?: () => void; "aria-expanded"?: boolean; "aria-haspopup"?: "dialog" | "listbox" }>;
  search: {
    placeholder: string;
    total?: ReactNode;
    value: string;
    onChange: (value: string) => void;
    /** `client`: the caller filters `sections` from the whole list. `server`: each change fetches a page, so the list may be longer than what is shown. */
    mode?: "client" | "server";
  };
  /** Called once when the list is scrolled to its end, to fetch the next page. */
  onEndReached?: () => void;
  /** A page is being fetched: shows a loading row at the end. */
  loading?: boolean;
  /** Filter tabs under the search (the conversation picker). */
  filters?: ReactNode;
  sections: PickerSection<T>[];
  getKey: (item: T) => string;
  renderItem: (item: T, state: { active: boolean; query: string }) => ReactNode;
  onSelect: (item: T) => void;
  /** "42 more · type to filter". */
  remainder?: ReactNode;
  /** Create agent / New conversation / New bundle · All bundles →. */
  footer?: ReactNode;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  /** Row height in px, for windowing long lists. */
  itemHeight?: number;
  /** Panel width in px. */
  width?: number;
  className?: string;
};

type Row<T> = { kind: "label"; label: ReactNode; key: string } | { kind: "item"; item: T; index: number; key: string };

const LABEL_HEIGHT = 28;
const MAX_HEIGHT = 360;
const OVERSCAN = 6;

/**
 * The searchable picker (X41), behind the agent, conversation, topic and
 * bundle pickers. ↑ ↓ move, Enter selects, Esc closes, typing filters.
 * Long lists are windowed, so a thousand rows cost the same as ten.
 */
export function Picker<T>({
  label,
  trigger,
  search,
  filters,
  sections,
  getKey,
  renderItem,
  onSelect,
  remainder,
  footer,
  onEndReached,
  loading,
  open: openProp,
  onOpenChange,
  itemHeight = 45,
  width = 340,
  className,
}: PickerProps<T>) {
  const id = useId();
  const [openState, setOpenState] = useState(false);
  const open = trigger ? (openProp ?? openState) : true;
  const setOpen = (v: boolean) => {
    setOpenState(v);
    onOpenChange?.(v);
  };
  const [active, setActive] = useState(0);
  const [scrollTop, setScrollTop] = useState(0);
  const list = useRef<HTMLDivElement>(null);
  const root = useRef<HTMLDivElement>(null);

  const { rows, items, offsets, height } = useMemo(() => {
    const rows: Row<T>[] = [];
    const items: T[] = [];
    sections.forEach((s, si) => {
      if (!s.items.length) return;
      if (s.label) rows.push({ kind: "label", label: s.label, key: `label-${si}` });
      s.items.forEach((item) => {
        rows.push({ kind: "item", item, index: items.length, key: getKey(item) });
        items.push(item);
      });
    });
    const offsets: number[] = [];
    let y = 0;
    for (const r of rows) {
      offsets.push(y);
      y += r.kind === "label" ? LABEL_HEIGHT : itemHeight;
    }
    return { rows, items, offsets, height: y };
  }, [sections, getKey, itemHeight]);

  useEffect(() => setActive(0), [search.value, sections]);

  // Ask for the next page when the end is in view (once per list length).
  const endedAt = useRef(-1);
  useEffect(() => {
    if (!onEndReached || loading || !open) return;
    const el = list.current;
    if (!el) return;
    const atEnd = el.scrollTop + el.clientHeight >= height - 2 * itemHeight;
    if (atEnd && endedAt.current !== items.length) {
      endedAt.current = items.length;
      onEndReached();
    }
  }, [scrollTop, height, items.length, loading, open, onEndReached, itemHeight]);

  // Keep the active row in view.
  useEffect(() => {
    const el = list.current;
    const row = rows.findIndex((r) => r.kind === "item" && r.index === active);
    if (!el || row < 0) return;
    const top = offsets[row];
    if (top < el.scrollTop) el.scrollTop = top;
    else if (top + itemHeight > el.scrollTop + el.clientHeight) el.scrollTop = top + itemHeight - el.clientHeight;
  }, [active, rows, offsets, itemHeight]);

  // Close on a click outside.
  useEffect(() => {
    if (!trigger || !open) return;
    const onDown = (e: PointerEvent) => {
      if (!root.current?.contains(e.target as Node) && !inFloating(e.target)) setOpen(false);
    };
    document.addEventListener("pointerdown", onDown);
    return () => document.removeEventListener("pointerdown", onDown);
  });

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((a) => Math.min(items.length - 1, a + 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => Math.max(0, a - 1));
    } else if (e.key === "Enter" && items[active] !== undefined) {
      e.preventDefault();
      onSelect(items[active]);
      if (trigger) setOpen(false);
    } else if (e.key === "Escape" && trigger) {
      e.preventDefault();
      setOpen(false);
    }
  };

  const viewport = Math.min(height, MAX_HEIGHT);
  let first = 0;
  while (first < rows.length - 1 && offsets[first + 1] <= scrollTop) first++;
  let last = first;
  while (last < rows.length - 1 && offsets[last] < scrollTop + viewport) last++;
  const from = Math.max(0, first - OVERSCAN);
  const to = Math.min(rows.length, last + OVERSCAN + 1);
  const listId = `${id}-list`;
  const optionId = (i: number) => `${id}-opt-${i}`;

  const panel = (
    <div
      className={cx(
        "flex flex-col overflow-hidden rounded-xl border border-border-strong bg-surface",
        trigger && "shadow-popover",
        className,
      )}
      style={{ width }}
    >
      <label className="flex items-center gap-8 border-b border-border px-14 py-12">
        <Icon name="search" size={15} tone="faint" />
        <input
          autoFocus={!!trigger}
          role="combobox"
          aria-expanded
          aria-controls={listId}
          aria-activedescendant={items.length ? optionId(active) : undefined}
          aria-label={search.placeholder}
          placeholder={search.placeholder}
          value={search.value}
          onChange={(e) => search.onChange(e.target.value)}
          onKeyDown={onKeyDown}
          className="min-w-0 flex-1 bg-transparent type-body text-fg caret-live outline-none placeholder:text-faint"
        />
        {search.total != null && <span className="type-mono-xs text-faint whitespace-nowrap">{search.total}</span>}
      </label>
      {filters && <div className="flex gap-4 px-10 pt-8">{filters}</div>}
      <div
        ref={list}
        id={listId}
        role="listbox"
        aria-label={label}
        onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
        className="relative overflow-y-auto"
        style={{ height: viewport }}
      >
        <div style={{ height, position: "relative" }}>
          {rows.slice(from, to).map((r, i) => {
            const at = offsets[from + i];
            if (r.kind === "label") {
              return (
                <div
                  key={r.key}
                  role="presentation"
                  className="absolute inset-x-0 flex items-end px-14 pb-4 type-label uppercase text-faint"
                  style={{ top: at, height: LABEL_HEIGHT }}
                >
                  {r.label}
                </div>
              );
            }
            const isActive = r.index === active;
            return (
              <div
                key={r.key}
                id={optionId(r.index)}
                role="option"
                aria-selected={isActive}
                onPointerMove={() => setActive(r.index)}
                onClick={() => {
                  onSelect(r.item);
                  if (trigger) setOpen(false);
                }}
                className={cx("absolute inset-x-0 flex cursor-pointer items-center px-14", isActive && "bg-raised")}
                style={{ top: at, height: itemHeight }}
              >
                {renderItem(r.item, { active: isActive, query: search.value })}
              </div>
            );
          })}
        </div>
        {loading && (
          <div role="status" className="flex justify-center py-10 type-small text-faint">
            Loading…
          </div>
        )}
      </div>
      {remainder && <p className="px-14 pt-6 pb-10 type-small text-faint">{remainder}</p>}
      {footer && <div className="flex items-center gap-16 border-t border-border px-14 py-10">{footer}</div>}
    </div>
  );

  if (!trigger) return panel;
  return (
    <div ref={root} className="relative inline-flex">
      {cloneElement(trigger, {
        onClick: () => setOpen(!open),
        "aria-expanded": open,
        "aria-haspopup": "dialog",
      })}
      {open && <Floating anchor={root}>{panel}</Floating>}
    </div>
  );
}

/** Bolds the part of `text` that matches the query, as picker rows do. */
export function HighlightMatch({ text, query }: { text: string; query: string }) {
  const q = query.trim();
  const at = q ? text.toLowerCase().indexOf(q.toLowerCase()) : -1;
  if (at < 0) return <>{text}</>;
  return (
    <>
      {text.slice(0, at)}
      <mark className="bg-transparent font-semibold text-fg">{text.slice(at, at + q.length)}</mark>
      {text.slice(at + q.length)}
    </>
  );
}

/** A footer action: "+ Create agent", "All bundles →". */
export function PickerAction({ icon, children, ...rest }: { icon?: "plus"; children: ReactNode } & React.ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button type="button" className="inline-flex items-center gap-8 rounded-xs type-ui-medium text-fg hover:underline" {...rest}>
      {icon && <Icon name={icon} size={14} />}
      {children}
    </button>
  );
}
