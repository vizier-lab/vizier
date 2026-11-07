import type { HTMLAttributes } from "react";
import { Icon } from "../../icons/Icon";
import { Sigil } from "../../brand/Sigil/Sigil";
import { cx } from "../../lib/cx";
import { Avatar } from "../Avatar/Avatar";
import { HighlightMatch } from "../Picker/Picker";

/** A file or folder of a project, or (in a Workspace) an agent or person to mention. */
export type MentionMenuItem = { path: string; kind: "file" | "dir" } | MentionMenuMember;

export type MentionMenuMember = {
  kind: "agent" | "person";
  /** What follows the "@": an agent id or a username. */
  id: string;
  name: string;
  /** An agent that is not healthy says so here ("restarting"); a person's role may go here too. */
  hint?: string;
  /** The agent's own picture or the person's photo. */
  imageSrc?: string;
};

function isMember(item: MentionMenuItem): item is MentionMenuMember {
  return item.kind === "agent" || item.kind === "person";
}

function keyOf(item: MentionMenuItem) {
  return isMember(item) ? `${item.kind}:${item.id}` : item.path;
}

export type MentionMenuProps = {
  /** What the list is of: "Files", or "Agents and people". */
  label: string;
  /** Entries matching what was typed after "@". */
  items: MentionMenuItem[];
  query: string;
  /** Index of the highlighted row. */
  active: number;
  onPick: (item: MentionMenuItem) => void;
  /** Said when nothing matches, before "Enter sends it as text". Defaults to the file wording. */
  emptyText?: string;
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
export function MentionMenu({ label, items, query, active, onPick, id, emptyText, className, ...rest }: MentionMenuProps) {
  return (
    <div className={cx("flex flex-col rounded-lg border border-border-strong surface-float p-6 shadow-popover", className)} {...rest}>
      <span className="px-8 pt-4 pb-4 type-label uppercase text-faint">{label}</span>
      {items.length === 0 ? (
        <p id={id} role="status" className="px-8 py-8 type-small text-muted">
          {emptyText ?? `No file matches “@${query}”.`} Enter sends it as text.
        </p>
      ) : (
        <div id={id} role="listbox" aria-label={label} className="flex max-h-240 flex-col overflow-y-auto">
          {items.map((item, i) => (
            <div
              key={keyOf(item)}
              id={`${id}-${i}`}
              role="option"
              aria-selected={i === active}
              onPointerDown={(e) => {
                e.preventDefault();
                onPick(item);
              }}
              className={cx("flex cursor-pointer items-center gap-8 rounded-sm px-8 py-6", i === active && "bg-raised")}
            >
              {isMember(item) ? <MemberRow item={item} query={query} /> : <FileRow item={item} query={query} />}
            </div>
          ))}
        </div>
      )}
      <p className="px-8 pt-4 pb-2 type-mono-xs text-faint">↑↓ to move · Tab or Enter to mention · Esc to close</p>
    </div>
  );
}

function FileRow({ item, query }: { item: { path: string; kind: "file" | "dir" }; query: string }) {
  const { dir, name } = split(item.path);
  return (
    <>
      <Icon name={item.kind === "dir" ? "folder" : "file"} size={14} tone="faint" />
      <span className="min-w-0 truncate type-code">
        {dir && <span className="text-muted">{dir}</span>}
        <span className="text-fg">
          <HighlightMatch text={name} query={query} />
        </span>
      </span>
    </>
  );
}

/** An agent (its sigil) or a person (their face), the name strong and the handle quiet. */
function MemberRow({ item, query }: { item: MentionMenuMember; query: string }) {
  return (
    <>
      {item.kind === "agent" ? (
        <Sigil size="sm" agentId={item.id} image={item.imageSrc} />
      ) : (
        <Avatar name={item.id} src={item.imageSrc} size="xs" square />
      )}
      <span className="min-w-0 truncate type-ui text-fg">
        <HighlightMatch text={item.name} query={query} />
      </span>
      <span className="min-w-0 truncate type-code text-muted">
        @<HighlightMatch text={item.id} query={query} />
      </span>
      {item.hint && <span className="ml-auto shrink-0 type-mono-xs text-faint">{item.hint}</span>}
    </>
  );
}
