import type { ReactNode } from "react";
import { ListItem, type ListItemProps } from "../ListItem/ListItem";

export type RecalledMemoryRowProps = {
  /** The passage address: "qdrant.md#3". */
  path: string;
  bundle: ReactNode;
  /** Similarity, 0–1; sizes the dot and shows as the meta. */
  score: number;
} & Omit<ListItemProps, "title" | "subtitle" | "meta" | "leading">;

/** A passage recalled into the turn (X29): dot sized by score, path, bundle, score. Built on ListItem. */
export function RecalledMemoryRow({ path, bundle, score, ...rest }: RecalledMemoryRowProps) {
  // 4px at no match up to 10px at a perfect one.
  const dot = 4 + Math.round(Math.max(0, Math.min(1, score)) * 6);
  return (
    <ListItem
      shape="bare"
      leading={
        <span aria-hidden className="flex size-14 items-center justify-center">
          <span className="rounded-pill bg-muted" style={{ width: dot, height: dot }} />
        </span>
      }
      title={
        <span className="flex items-center gap-8">
          <span className="truncate type-mono-sm text-fg">{path}</span>
          <span className="type-mono-xs text-faint">{bundle}</span>
        </span>
      }
      meta={<span className="type-mono-sm-medium text-muted">{score.toFixed(2)}</span>}
      {...rest}
    />
  );
}
