import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { Eyebrow } from "../Eyebrow/Eyebrow";

export type OutlineItem = { id: string; label: ReactNode; level: 1 | 2 | 3 };

export type OutlineNavProps = {
  items: OutlineItem[];
  /** The heading in view. */
  activeId?: string;
  onSelect: (id: string) => void;
} & Omit<HTMLAttributes<HTMLElement>, "onSelect">;

const INDENT = { 1: "pl-8", 2: "pl-20", 3: "pl-32" };

/** A document's headings, by level (X51). */
export function OutlineNav({ items, activeId, onSelect, className, ...rest }: OutlineNavProps) {
  return (
    <nav aria-label="Outline" className={cx("flex flex-col gap-8", className)} {...rest}>
      <Eyebrow>Outline</Eyebrow>
      <ul className="flex flex-col gap-2">
        {items.map((it) => {
          const active = it.id === activeId;
          return (
            <li key={it.id}>
              <button
                type="button"
                aria-current={active ? "location" : undefined}
                onClick={() => onSelect(it.id)}
                className={cx(
                  "w-full truncate rounded-sm py-4 pr-8 text-left",
                  INDENT[it.level],
                  it.level === 1 ? "type-ui-medium" : "type-ui",
                  active ? "bg-raised text-fg" : it.level === 3 ? "text-faint hover:text-fg" : "text-muted hover:text-fg",
                )}
              >
                {it.label}
              </button>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
