import { useRef, useState, type HTMLAttributes } from "react";
import { cx } from "../../lib/cx";
import { useDismiss } from "../../lib/dismiss";
import { Badge } from "../Badge/Badge";
import { Button } from "../Button/Button";
import { Floating } from "../Floating/Floating";
import { IconButton } from "../IconButton/IconButton";
import { IntegrationLogo } from "../IntegrationLogo/IntegrationLogo";
import { Menu } from "../Menu/Menu";
import { MenuItem } from "../MenuItem/MenuItem";

export type ConnectedAccountRowProps = {
  platform: "discord" | "telegram";
  /** connected: proven with a code. unverified: typed in by hand earlier. none: nothing yet. */
  state: "connected" | "unverified" | "none";
  /** The platform handle, without "@". A Telegram account may have none. */
  handle?: string | null;
  accountId?: string | null;
  /** When it was connected: "Connected 27 Nov 2026". */
  since?: string;
  /** connected: the row menu's Unlink. The caller confirms before unlinking. */
  onUnlink?: () => void;
  /** unverified: remove what was typed. */
  onRemove?: () => void;
  /** The removal is running. */
  busy?: boolean;
} & HTMLAttributes<HTMLDivElement>;

const NAMES = { discord: "Discord", telegram: "Telegram" };

/** One platform on the profile's Connected accounts card (X94). */
export function ConnectedAccountRow({
  platform,
  state,
  handle,
  accountId,
  since,
  onUnlink,
  onRemove,
  busy,
  className,
  ...rest
}: ConnectedAccountRowProps) {
  const name = NAMES[platform];
  const [open, setOpen] = useState(false);
  const trigger = useRef<HTMLSpanElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  useDismiss(open, () => setOpen(false), menu, trigger);

  const identity = [handle ? `@${handle}` : null, accountId].filter(Boolean).join("  ·  ");

  return (
    <div className={cx("flex items-center gap-12 rounded-lg border border-border bg-ground px-12 py-10", className)} {...rest}>
      <IntegrationLogo kind={platform} size={32} className={state === "connected" ? undefined : "opacity-40"} />
      <div className="flex min-w-0 flex-1 flex-col gap-2">
        <div className="flex items-center gap-8">
          <span className="type-ui-strong text-fg">{name}</span>
          {state === "unverified" && <Badge tone="retrying">Not verified</Badge>}
        </div>
        {identity && <span className="truncate type-mono-xs text-muted">{identity}</span>}
        {state === "unverified" && (
          <span className="type-small text-muted">Typed in earlier. Send /connect to confirm it.</span>
        )}
        {state === "none" && (
          <span className="type-small text-faint">
            Not connected. Send /connect to any agent&apos;s bot on {name}.
          </span>
        )}
        {/* A narrow screen has no room beside the name, so the date sits under it. */}
        {state === "connected" && since && <span className="type-small text-faint sm:hidden">{since}</span>}
      </div>
      {state === "connected" && since && <span className="hidden shrink-0 type-small text-faint sm:block">{since}</span>}
      {state === "connected" && (
        <span ref={trigger} className="shrink-0">
          <IconButton
            variant="ghost"
            size="md"
            icon="more"
            aria-label={`More for ${name}`}
            aria-haspopup="menu"
            aria-expanded={open}
            selected={open}
            onClick={() => setOpen((v) => !v)}
          />
        </span>
      )}
      {open && (
        <Floating ref={menu} anchor={trigger} align="end">
          <Menu aria-label={`${name} account`} className="w-160!">
            <MenuItem
              autoFocus
              icon="unlink"
              label="Unlink"
              tone="danger"
              onClick={() => {
                setOpen(false);
                onUnlink?.();
              }}
            />
          </Menu>
        </Floating>
      )}
      {state === "unverified" && (
        <Button size="sm" className="shrink-0" loading={busy} onClick={onRemove}>
          Remove
        </Button>
      )}
    </div>
  );
}
