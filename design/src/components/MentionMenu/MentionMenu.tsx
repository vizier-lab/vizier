import type { ComponentProps, HTMLAttributes } from "react";
import { Icon } from "../../icons/Icon";
import { Sigil } from "../../brand/Sigil/Sigil";
import { cx } from "../../lib/cx";
import { Avatar } from "../Avatar/Avatar";
import { HighlightMatch } from "../Picker/Picker";
import { DeckFrame } from "../ComposerDeck/DeckFrame";

/** A file or folder of a project, an agent or person to mention, or an entity of the chat's agent. */
export type MentionMenuItem = { path: string; kind: "file" | "dir" } | MentionMenuMember | MentionMenuEntity;

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

/** One entity of the chat's agent (specs/066): what `[[kind:name]]` addresses. */
export type MentionMenuEntity = {
  kind: "artifact" | "function" | "skill" | "task" | "memory";
  /** The entity's own name, as the link addresses it. */
  name: string;
  /** What the row says quietly on the right: the kind, or a skill's scope. */
  note?: string;
};

function isMember(item: MentionMenuItem): item is MentionMenuMember {
  return item.kind === "agent" || item.kind === "person";
}

function isEntity(item: MentionMenuItem): item is MentionMenuEntity {
  return item.kind === "artifact" || item.kind === "function" || item.kind === "skill" || item.kind === "task" || item.kind === "memory";
}

function keyOf(item: MentionMenuItem) {
  if (isMember(item)) return `${item.kind}:${item.id}`;
  if (isEntity(item)) return `entity:${item.kind}:${item.name}`;
  return item.path;
}

export type MentionMenuProps = {
  /** What the list is of: "Files", "Agents and people", or "Links". */
  label: string;
  /** Entries matching what was typed after "@" or "[[". */
  items: MentionMenuItem[];
  query: string;
  /** Index of the highlighted row. */
  active: number;
  onPick: (item: MentionMenuItem) => void;
  /** Said when nothing matches, before "Enter sends it as text". Defaults to the file wording. */
  emptyText?: string;
  /** The footer line; defaults to the mention wording. */
  footer?: string;
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
export function MentionMenu({ label, items, query, active, onPick, id, emptyText, footer, className, ...rest }: MentionMenuProps) {
  return (
    <DeckFrame label={label} footer={footer ?? "↑↓ to move · Tab or Enter to mention · Esc to close"} className={className} {...rest}>
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
              className={cx("flex cursor-pointer items-center gap-8 rounded-sm px-8 py-6", i === active && "bg-live text-on-live [&_*]:text-on-live!")}
            >
              {isMember(item) ? <MemberRow item={item} query={query} /> : isEntity(item) ? <EntityRow item={item} query={query} /> : <FileRow item={item} query={query} />}
            </div>
          ))}
        </div>
      )}
    </DeckFrame>
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

const ENTITY_ICON: Record<MentionMenuEntity["kind"], ComponentProps<typeof Icon>["name"]> = {
  artifact: "file",
  function: "functions",
  skill: "skills",
  task: "tasks",
  memory: "memory",
};

/** An entity of the chat's agent (specs/066): the kind's icon, the name strong, the note quiet. */
function EntityRow({ item, query }: { item: MentionMenuEntity; query: string }) {
  return (
    <>
      <Icon name={ENTITY_ICON[item.kind]} size={14} tone="faint" />
      <span className="min-w-0 truncate type-code text-fg">
        <HighlightMatch text={item.name} query={query} />
      </span>
      {item.note && <span className="ml-auto shrink-0 type-mono-xs text-faint">{item.note}</span>}
    </>
  );
}
