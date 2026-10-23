import type { ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { ListItem, type ListItemProps } from "../ListItem/ListItem";
import { HighlightMatch } from "../Picker/Picker";

export type ConversationKind = "webui" | "discord" | "telegram";

export type ConversationRowProps = {
  kind: ConversationKind;
  title: string;
  /** What matched in the search, shown bold in the title. */
  query?: string;
  /** Under the title: "WebUI · topic 3 of 5", "Discord · Vizier Lab". */
  place: ReactNode;
  /** A turn is running in it now. */
  live?: boolean;
  /** "14:02", "Yesterday". */
  time: ReactNode;
} & Omit<ListItemProps, "title" | "subtitle" | "meta" | "leading" | "trailing">;

const ICON = { webui: "chat", discord: "channels", telegram: "paper-plane" } as const;
const KIND = { webui: "WebUI", discord: "Discord", telegram: "Telegram" } as const;

/** A conversation in the conversation picker (X80): where it is, what it is called, and when it last moved. Built on ListItem. */
export function ConversationRow({ kind, title, query = "", place, live, time, shape = "bare", className, ...rest }: ConversationRowProps) {
  return (
    <ListItem
      shape={shape}
      className={className ?? "w-full"}
      leading={<Icon name={ICON[kind]} tone="muted" title={KIND[kind]} />}
      title={
        <span className="flex items-center gap-6">
          <span className="truncate">
            <HighlightMatch text={title} query={query} />
          </span>
          {live && <span role="img" aria-label="a turn is running" className="size-6 shrink-0 rounded-pill bg-live" />}
        </span>
      }
      subtitle={<span className="type-mono-xs text-faint">{place}</span>}
      meta={time}
      {...rest}
    />
  );
}
