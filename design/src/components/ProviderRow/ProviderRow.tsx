import type { ReactNode } from "react";
import { Badge } from "../Badge/Badge";
import { Button } from "../Button/Button";
import { IconButton } from "../IconButton/IconButton";
import { ListItem, type ListItemProps } from "../ListItem/ListItem";
import { ProviderLogo } from "../ProviderLogo/ProviderLogo";

export type ProviderRowProps = {
  provider: string;
  name: string;
  /** "Saved key ••••7c1e", "http://localhost:11434 · no key". */
  credential: ReactNode;
  /** "3 agents · 41 models", "not used yet". */
  usage: ReactNode;
  custom?: boolean;
  onModels: () => void;
  onMore: () => void;
} & Omit<ListItemProps, "title" | "subtitle" | "meta" | "leading" | "trailing">;

/** A connected provider (X64): logo, name, credential, usage, Models and ⋯. Built on ListItem. */
export function ProviderRow({ provider, name, credential, usage, custom, onModels, onMore, ...rest }: ProviderRowProps) {
  return (
    <ListItem
      shape="flush"
      leading={<ProviderLogo provider={provider} name={name} />}
      title={
        <span className="flex items-center gap-8">
          <span className="type-body-strong">{name}</span>
          {custom && <Badge>custom</Badge>}
        </span>
      }
      subtitle={<span className="truncate type-mono-xs text-faint">{credential}</span>}
      trailing={
        <>
          <span className="type-ui text-muted">{usage}</span>
          <Button size="sm" onClick={onModels}>Models</Button>
          <IconButton variant="ghost" size="sm" icon="more" aria-label={`More for ${name}`} onClick={onMore} />
        </>
      }
      {...rest}
    />
  );
}
