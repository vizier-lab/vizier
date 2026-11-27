import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { Avatar } from "../Avatar/Avatar";
import { Button } from "../Button/Button";
import { IntegrationLogo } from "../IntegrationLogo/IntegrationLogo";
import { Notice } from "../Notice/Notice";
import { Skeleton } from "../Skeleton/Skeleton";

export type ConnectAccount = {
  platform: "discord" | "telegram";
  accountId: string;
  /** Without "@". */
  handle?: string | null;
  displayName?: string | null;
};

export type ConnectConfirmProps = {
  /**
   * loading: reading the code. ready: waiting for the person's confirmation. unavailable: unknown,
   * expired or used. elsewhere: the account is someone else's. failed: the code could not be read.
   */
  state: "loading" | "ready" | "unavailable" | "elsewhere" | "failed";
  /** ready: the platform account the code belongs to. */
  account?: ConnectAccount;
  /** ready: the signed-in Vizier user it would be connected to. */
  username?: string;
  /** ready: the person's current account on this platform, which confirming unlinks. */
  replaces?: ConnectAccount | null;
  /** What went wrong, in the server's words. */
  message?: string;
  /** ready: the connection is being made. */
  busy?: boolean;
  onConfirm?: () => void;
  /** Cancel, or leave a state that has nothing to confirm. */
  onCancel: () => void;
  /** failed: read the code again. */
  onRetry?: () => void;
  /** ready: "Not you? Sign out". Absent where there is no signing out. */
  onSignOut?: () => void;
  /** The leaving button of a state with nothing to confirm. */
  doneLabel?: string;
} & HTMLAttributes<HTMLDivElement>;

const NAMES = { discord: "Discord", telegram: "Telegram" };

/** The heading for a state, for whatever holds the panel: a page or a dialog. */
export function connectConfirmTitle({ state, account }: Pick<ConnectConfirmProps, "state" | "account">): string {
  if (state === "ready" && account) return `Connect your ${NAMES[account.platform]} account`;
  if (state === "unavailable") return "This code no longer works";
  if (state === "elsewhere") return "Can't connect this account";
  if (state === "failed") return "Couldn't check the code";
  return "Checking the code";
}

/** "@rin_dev (id 381926457021)", or the id alone. */
function named({ handle, accountId }: ConnectAccount) {
  return handle ? `@${handle} (id ${accountId})` : `id ${accountId}`;
}

function Tile({ leading, children, trailing }: { leading: ReactNode; children: ReactNode; trailing?: ReactNode }) {
  return (
    <div className="flex items-center gap-12 rounded-lg border border-border bg-ground px-14 py-10">
      {leading}
      <div className="flex min-w-0 flex-1 flex-col gap-2">{children}</div>
      {trailing}
    </div>
  );
}

/**
 * Confirming a connect code (X95): which platform account would be connected to which Vizier user.
 * Nothing is connected until the person presses the button.
 */
export function ConnectConfirm({
  state,
  account,
  username,
  replaces,
  message,
  busy,
  onConfirm,
  onCancel,
  onRetry,
  onSignOut,
  doneLabel = "Go to Profile",
  className,
  ...rest
}: ConnectConfirmProps) {
  const body = () => {
    if (state === "ready" && account) {
      const platform = NAMES[account.platform];
      const name = [account.displayName, account.handle ? `@${account.handle}` : null].filter(Boolean).join("  ·  ");
      return (
        <>
          <Tile leading={<IntegrationLogo kind={account.platform} size={32} />}>
            <span className="truncate type-ui-strong text-fg">{name || `${platform} account`}</span>
            <span className="truncate type-mono-xs text-muted">
              {platform} id {account.accountId}
            </span>
          </Tile>
          {username && (
            <div className="flex flex-col gap-6">
              <span className="type-small text-faint">to your Vizier user</span>
              <Tile
                leading={<Avatar name={username} size="sm" />}
                trailing={
                  onSignOut && (
                    <Button variant="ghost" size="sm" onClick={onSignOut}>
                      Not you? Sign out
                    </Button>
                  )
                }
              >
                <span className="truncate type-ui-strong text-fg">{username}</span>
              </Tile>
            </div>
          )}
          {replaces && (
            <Notice tone="info">
              This replaces your current {platform} account, {named(replaces)}. It will be unlinked.
            </Notice>
          )}
          <Notice tone="retrying" icon="alert">
            Only confirm a code you asked for yourself. After this, messages from this {platform} account read as yours.
          </Notice>
          {message && <Notice tone="failed">{message}</Notice>}
          <div className="flex gap-10 pt-4">
            <Button className="flex-1" onClick={onCancel} disabled={busy}>
              Cancel
            </Button>
            <Button className="flex-1" variant="primary" loading={busy} onClick={onConfirm}>
              {replaces ? "Replace account" : "Connect account"}
            </Button>
          </div>
        </>
      );
    }
    if (state === "loading") {
      return (
        <div role="status" aria-label="Checking the code" className="flex flex-col gap-12">
          <Skeleton className="h-56" />
          <Skeleton className="h-56" />
        </div>
      );
    }
    return (
      <>
        {state === "failed" ? (
          <Notice tone="failed">{message ?? "Something went wrong."}</Notice>
        ) : state === "elsewhere" ? (
          <p className="type-ui-para text-muted">
            {message ?? "This account is already connected to another Vizier user."} They must unlink it first.
          </p>
        ) : (
          <p className="type-ui-para text-muted">
            {message ?? "This code has expired or was already used. Send /connect again."}
          </p>
        )}
        <div className="flex gap-10 pt-4">
          <Button onClick={onCancel}>{doneLabel}</Button>
          {state === "failed" && onRetry && (
            <Button variant="primary" onClick={onRetry}>
              Try again
            </Button>
          )}
        </div>
      </>
    );
  };

  return (
    <div className={cx("flex flex-col gap-14", className)} {...rest}>
      {body()}
    </div>
  );
}
