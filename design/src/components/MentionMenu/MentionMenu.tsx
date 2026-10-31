import type { HTMLAttributes } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { HighlightMatch } from "../Picker/Picker";

export type MentionMenuItem = { path: string; kind: "file" | "dir" };

export type MentionMenuProps = {
  /** What the list is of: "Files". */
  label: string;
  /** Entries matching what was typed after "@". */
  items: MentionMenuItem[];
  query: string;
  /** Index of the highlighted row. */
  active: number;
  onPick: (item: MentionMenuItem) => void;
  /** The composer's text field controls this list via aria-activedescendant. */
  id: string;
} & Omit<HTMLAttributes<HTMLDivElement>, "id">;

/** The folder part of a path and its last component: "src/cli/" + "agent.rs". */
function split(path: string) {
  const trimmed = path.endsWith("/") ? path.slice(0, -1) : path;
  const at = trimmed.lastIndexOf("/");
  return at < 0 ? { dir: "", name: path } : { dir: path.slice(0, at + 1), name: path.slice(at + 1) };
}

/**
 * The "@" menu above the composer: files and folders of the chat's working folder, filtered as
 * you type. Built like the slash menu (SkillMenu); the composer owns the keys (↑ ↓ Tab Enter Esc).
 * The folder is drawn quiet and the name strong, so the eye finds the file, not the path.
 */
export function MentionMenu({ label, items, query, active, onPick, id, className, ...rest }: MentionMenuProps) {
  return (
    <div className={cx("flex flex-col rounded-lg border border-border-strong bg-surface p-6 shadow-popover", className)} {...rest}>
      <span className="px-8 pt-4 pb-4 type-label uppercase text-faint">{label}</span>
      {items.length === 0 ? (
        <p id={id} role="status" className="px-8 py-8 type-small text-muted">
          No file matches “@{query}”. Enter sends it as text.
        </p>
      ) : (
        <div id={id} role="listbox" aria-label={label} className="flex max-h-240 flex-col overflow-y-auto">
          {items.map((item, i) => {
            const { dir, name } = split(item.path);
            return (
              <div
                key={item.path}
                id={`${id}-${i}`}
                role="option"
                aria-selected={i === active}
                onPointerDown={(e) => {
                  e.preventDefault();
                  onPick(item);
                }}
                className={cx("flex cursor-pointer items-center gap-8 rounded-sm px-8 py-6", i === active && "bg-raised")}
              >
                <Icon name={item.kind === "dir" ? "folder" : "file"} size={14} tone="faint" />
                <span className="min-w-0 truncate type-code">
                  {dir && <span className="text-muted">{dir}</span>}
                  <span className="text-fg">
                    <HighlightMatch text={name} query={query} />
                  </span>
                </span>
              </div>
            );
          })}
        </div>
      )}
      <p className="px-8 pt-4 pb-2 type-mono-xs text-faint">↑↓ to move · Tab or Enter to mention · Esc to close</p>
    </div>
  );
}
