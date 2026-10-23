import { useMemo, useRef, useState, type HTMLAttributes, type KeyboardEvent, type ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons/map";
import { cx } from "../../lib/cx";

export type FileTreeNode = {
  id: string;
  label: ReactNode;
  /** A folder opens; a file is chosen; an action ("Load 40 more") runs `onSelect` and is never selected. */
  kind: "folder" | "file" | "action";
  /** Replaces the folder/file glyph: a memory bundle, a skill. */
  icon?: IconName;
  /** Faint, after the label: a count, a time. */
  meta?: ReactNode;
  /** Monospace label, for paths and slugs. */
  mono?: boolean;
  /** A folder's contents; `undefined` while they load. */
  children?: FileTreeNode[];
};

export type FileTreeProps = {
  nodes: FileTreeNode[];
  /** Ids of the open folders. */
  expanded: ReadonlySet<string>;
  onToggle: (id: string, open: boolean) => void;
  selectedId?: string;
  onSelect: (node: FileTreeNode) => void;
  /** The tree's accessible name. */
  label: string;
  /** Shown when there are no nodes at all. */
  empty?: ReactNode;
} & Omit<HTMLAttributes<HTMLDivElement>, "onSelect" | "onToggle">;

type Row = { node: FileTreeNode; depth: number; parent?: string } | { loading: true; id: string; depth: number; parent: string };

function flatten(nodes: FileTreeNode[], expanded: ReadonlySet<string>, depth = 0, parent?: string, out: Row[] = []): Row[] {
  for (const node of nodes) {
    out.push({ node, depth, parent });
    if (node.kind !== "folder" || !expanded.has(node.id)) continue;
    if (node.children === undefined) out.push({ loading: true, id: `${node.id}\u0000loading`, depth: depth + 1, parent: node.id });
    else flatten(node.children, expanded, depth + 1, node.id, out);
  }
  return out;
}

/**
 * A file explorer: folders that open in place, files to choose (X93). Memory's bundles and their
 * documents. Arrow keys move, Right opens, Left closes or climbs to the parent, Enter chooses.
 */
export function FileTree({ nodes, expanded, onToggle, selectedId, onSelect, label, empty, className, ...rest }: FileTreeProps) {
  const rows = useMemo(() => flatten(nodes, expanded), [nodes, expanded]);
  const items = rows.filter((r): r is Extract<Row, { node: FileTreeNode }> => !("loading" in r));
  const [focus, setFocus] = useState<string>();
  const refs = useRef(new Map<string, HTMLDivElement>());
  // The roving tab stop: the focused row, else the selected one, else the first.
  const stop = items.some((r) => r.node.id === focus) ? focus : items.some((r) => r.node.id === selectedId) ? selectedId : items[0]?.node.id;

  const move = (id: string | undefined) => {
    if (!id) return;
    setFocus(id);
    refs.current.get(id)?.focus();
  };

  const activate = (node: FileTreeNode) => {
    if (node.kind === "folder") onToggle(node.id, !expanded.has(node.id));
    onSelect(node);
  };

  const onKeyDown = (e: KeyboardEvent, i: number) => {
    const row = items[i];
    const { node } = row;
    const open = node.kind === "folder" && expanded.has(node.id);
    switch (e.key) {
      case "ArrowDown":
        move(items[i + 1]?.node.id);
        break;
      case "ArrowUp":
        move(items[i - 1]?.node.id);
        break;
      case "Home":
        move(items[0]?.node.id);
        break;
      case "End":
        move(items[items.length - 1]?.node.id);
        break;
      case "ArrowRight":
        if (node.kind !== "folder") return;
        if (!open) onToggle(node.id, true);
        else move(items[i + 1]?.depth === row.depth + 1 ? items[i + 1].node.id : undefined);
        break;
      case "ArrowLeft":
        if (open) onToggle(node.id, false);
        else move(row.parent);
        break;
      case "Enter":
      case " ":
        activate(node);
        break;
      default:
        return;
    }
    e.preventDefault();
  };

  if (nodes.length === 0 && empty) return <div className={className}>{empty}</div>;

  let index = -1;
  return (
    <div role="tree" aria-label={label} className={cx("flex flex-col", className)} {...rest}>
      {rows.map((row) => {
        const indent = { paddingLeft: 8 + row.depth * 16 };
        if ("loading" in row) {
          return (
            <div key={row.id} role="none" style={indent} className="flex h-32 items-center gap-8 pr-8 type-small text-faint">
              <span className="w-15" />
              Loading…
            </div>
          );
        }
        index += 1;
        const i = index;
        const { node, depth } = row;
        const folder = node.kind === "folder";
        const open = folder && expanded.has(node.id);
        const selected = node.id === selectedId;
        return (
          <div
            key={node.id}
            ref={(el) => {
              if (el) refs.current.set(node.id, el);
              else refs.current.delete(node.id);
            }}
            role="treeitem"
            aria-level={depth + 1}
            aria-expanded={folder ? open : undefined}
            aria-selected={node.kind === "action" ? undefined : selected}
            tabIndex={node.id === stop ? 0 : -1}
            style={indent}
            onFocus={() => setFocus(node.id)}
            onClick={() => activate(node)}
            onKeyDown={(e) => onKeyDown(e, i)}
            className={cx(
              // The ring sits inside the row, so a scrolling pane never clips it.
              "group flex h-32 cursor-pointer items-center gap-8 rounded-sm pr-8 select-none focus-visible:-outline-offset-2",
              selected ? "bg-raised text-fg" : "text-muted hover:bg-raised/60 hover:text-fg",
            )}
          >
            {folder ? (
              <Icon name="chevron-right" size={13} className={cx("text-faint transition-transform duration-150", open && "rotate-90")} />
            ) : (
              <span className="w-13 shrink-0" />
            )}
            {node.kind === "action" ? null : (
              <Icon name={node.icon ?? (folder ? "folder" : "file")} size={15} className={selected ? "text-fg" : "text-faint group-hover:text-muted"} />
            )}
            <span className={cx("min-w-0 flex-1 truncate", node.kind === "action" ? "type-small text-faint group-hover:text-fg" : node.mono ? "type-mono-sm" : "type-ui")}>
              {node.label}
            </span>
            {node.meta !== undefined && <span className="shrink-0 type-mono-xs text-faint">{node.meta}</span>}
          </div>
        );
      })}
    </div>
  );
}
