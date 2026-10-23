import type { ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { Status, type StatusState } from "../Status/Status";
import { Switch } from "../Switch/Switch";
import { TableRow, type TableRowProps } from "../TableRow/TableRow";

export type ServerRowProps = {
  name: string;
  transport: "remote" | "local";
  /** URL or command line. */
  target: string;
  state: Extract<StatusState, "connected" | "reconnecting" | "disabled" | "failed">;
  /** "timed out listing tools · retry in 20s". */
  detail?: ReactNode;
  /** The category its tools appear under (`<key>_mcp` when the key is taken). */
  category: string;
  tools?: number;
  enabled: boolean;
  onEnabledChange: (enabled: boolean) => void;
} & Omit<TableRowProps, "cells">;

/** An MCP server in the servers table (X62). Columns: server, status, category, tools, on. */
export function ServerRow({ name, transport, target, state, detail, category, tools, enabled, onEnabledChange, ...rest }: ServerRowProps) {
  return (
    <TableRow
      cells={[
        <div className="flex items-start gap-10">
          <Icon name={transport === "remote" ? "globe" : "square-terminal"} size={15} tone="muted" title={transport === "remote" ? "remote" : "local command"} className="mt-2" />
          <div className="flex min-w-0 flex-col gap-2">
            <span className="type-code-medium text-fg">{name}</span>
            <span className="type-mono-xs break-all text-faint">{target}</span>
          </div>
        </div>,
        <div className="flex flex-col gap-2">
          <Status state={state} font="sans" />
          {detail && <span className="type-small text-faint">{detail}</span>}
        </div>,
        <span className="type-mono-sm text-muted">{category}</span>,
        <span className="type-mono-sm text-muted">{tools ?? "–"}</span>,
        <Switch checked={enabled} onCheckedChange={onEnabledChange} aria-label={`${name} enabled`} />,
      ]}
      {...rest}
    />
  );
}
