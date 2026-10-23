import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";
import { HighlightMatch } from "../Picker/Picker";

export type SkillMenuItem = { name: string; description: string };

export type SkillMenuProps = {
  /** Skills matching what was typed after "/". */
  items: SkillMenuItem[];
  query: string;
  /** Index of the highlighted row. */
  active: number;
  onPick: (item: SkillMenuItem) => void;
  /** The composer's text field controls this list via aria-activedescendant. */
  id: string;
} & Omit<HTMLAttributes<HTMLDivElement>, "id">;

/**
 * The slash menu above the composer (X33): built-in commands and the agent's
 * skills, filtered as you type. The composer owns the keys (↑ ↓ Tab Enter).
 */
export function SkillMenu({ items, query, active, onPick, id, className, ...rest }: SkillMenuProps) {
  return (
    <div className={cx("flex flex-col rounded-lg border border-border-strong bg-surface p-6 shadow-popover", className)} {...rest}>
      <span className="px-8 pt-4 pb-4 type-label uppercase text-faint">Skills</span>
      {items.length === 0 ? (
        <p id={id} role="status" className="px-8 py-8 type-small text-muted">
          No skill matches “/{query}”. Enter sends it as a message.
        </p>
      ) : (
        <div id={id} role="listbox" aria-label="Skills" className="flex flex-col">
          {items.map((s, i) => (
            <div
              key={s.name}
              id={`${id}-${i}`}
              role="option"
              aria-selected={i === active}
              onPointerDown={(e) => {
                e.preventDefault();
                onPick(s);
              }}
              className={cx("flex cursor-pointer items-center gap-10 rounded-sm px-8 py-6", i === active && "bg-raised")}
            >
              <span className="type-code-medium text-fg">
                /<HighlightMatch text={s.name} query={query} />
              </span>
              <span className="min-w-0 truncate type-small text-muted">{s.description}</span>
            </div>
          ))}
        </div>
      )}
      <p className="px-8 pt-4 pb-2 type-mono-xs text-faint">↑↓ to move · Tab to complete · Enter to run</p>
    </div>
  );
}
