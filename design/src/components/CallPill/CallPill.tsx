import type { HTMLAttributes } from "react";
import { Icon } from "../../icons/Icon";
import { DotLoader } from "../DotLoader/DotLoader";
import { useRef, useState } from "react";
import { Floating } from "../Floating/Floating";
import { TooltipBubble } from "../Tooltip/Tooltip";
import { cx } from "../../lib/cx";

/** Where a live voice call is (spec 055). */
export type CallState =
  | "connecting"
  | "listening"
  | "speaking"
  | "interrupted"
  | "reconnecting"
  | "ended"
  | "failed";

export type CallPillProps = {
  state: CallState;
  /** Seconds since the call started, shown as m:ss. */
  elapsedSeconds?: number;
  /** The sound level, 0 to 1: the microphone while listening, the reply while speaking. */
  level?: number;
  /** Why the call ended or failed, in a few words: "microphone blocked". */
  reason?: string;
  /** Ends the call. Offered while it runs. */
  onEnd?: () => void;
  /** Starts a new call after an ending. */
  onRestart?: () => void;
  /** Puts the ended pill away. */
  onDismiss?: () => void;
} & Omit<HTMLAttributes<HTMLDivElement>, "children">;

const WORD: Record<CallState, string> = {
  connecting: "Connecting",
  listening: "Listening",
  speaking: "Speaking",
  interrupted: "Interrupted",
  reconnecting: "Reconnecting",
  ended: "Call ended",
  failed: "Call failed",
};

/** The word a call's state is read as, here and wherever a host names it. */
export function callWord(state: CallState): string {
  return WORD[state];
}

/** A fixed shape, taller in the middle; the level lifts it. Five bars read as sound, not data. */
const SHAPE = [0.35, 0.7, 1, 0.6, 0.3];

function clock(total: number): string {
  const s = Math.max(0, Math.floor(total));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

function Meter({ level }: { level: number }) {
  const loud = Math.max(0, Math.min(1, level));
  return (
    <span aria-hidden className="flex h-16 items-center gap-2 max-lg:hidden">
      {SHAPE.map((shape, i) => (
        <span
          key={i}
          className="w-2 rounded-pill bg-live transition-[height] duration-(--vz-duration-fast) motion-reduce:transition-none"
          style={{ height: Math.round(4 + shape * (4 + loud * 8)) }}
        />
      ))}
    </span>
  );
}

/**
 * The stop control. Its label is drawn through `Floating`, not the plain `Tooltip`: the pill sits in
 * the deck's lip, which clips anything that leaves it, and a portal is the one way out.
 */
function StopButton({ onEnd }: { onEnd: () => void }) {
  const anchor = useRef<HTMLSpanElement>(null);
  const [open, setOpen] = useState(false);
  return (
    <span
      ref={anchor}
      className="inline-flex"
      onPointerEnter={() => setOpen(true)}
      onPointerLeave={() => setOpen(false)}
      onFocus={() => setOpen(true)}
      onBlur={() => setOpen(false)}
      onKeyDown={(e) => e.key === "Escape" && setOpen(false)}
    >
      <button type="button" aria-label="End live voice" onClick={onEnd} className={pillButton}>
        <span aria-hidden className="size-10 rounded-xs bg-current" />
      </button>
      {open && (
        <Floating anchor={anchor} align="start">
          <TooltipBubble label="End live voice" className="animate-fade-in motion-reduce:animate-none" />
        </Floating>
      )}
    </span>
  );
}

const pillButton =
  "flex size-20 shrink-0 items-center justify-center rounded-pill text-fg hover:bg-raised focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-focus";

/**
 * A live voice call in the composer deck's lip (spec 055, drawn on "Composer · two panels"). It
 * sits in the slot `/btw` uses and never opens the deck: what is said and answered lands in the
 * conversation, and what the agent does meanwhile shows in its activity trail, never here. While the call runs: a dot, the state word, a level meter, the time and a stop
 * control. After it ends: the reason, Start again and a close. On a phone the meter and the
 * divider give way so the strip still fits beside it.
 */
export function CallPill({
  state,
  elapsedSeconds = 0,
  level = 0,
  reason,
  onEnd,
  onRestart,
  onDismiss,
  className,
  ...rest
}: CallPillProps) {
  const over = state === "ended" || state === "failed";
  const waiting = state === "connecting";
  const dot =
    state === "reconnecting" ? "bg-retrying animate-shimmer" : waiting ? "bg-live animate-shimmer" : "bg-live";

  return (
    <div
      className={cx(
        "inline-flex h-24 shrink-0 items-center gap-6 rounded-pill border pl-10",
        over ? "border-border bg-raised pr-4" : "border-border-strong bg-surface pr-2",
        className,
      )}
      {...rest}
    >
      {over ? (
        <span role="status" data-state={state} className="flex min-w-0 items-center gap-4 type-small whitespace-nowrap">
          {/* On a phone a reason speaks for itself ("microphone is blocked"), so the word gives way
              to it; without one the word stays. The status still names the state for a reader. */}
          <span className={cx("font-medium text-muted", reason && "max-lg:sr-only")}>{WORD[state]}</span>
          {reason && (
            <>
              <span aria-hidden className="text-faint max-lg:hidden">·</span>
              {/* A reason is a few words; past 240px (160px on a phone) it is cut and read whole on
                  hover, so the strip and the model switcher beside the pill keep their room. */}
              <span title={reason} className={cx("max-w-240 truncate max-lg:max-w-160", state === "failed" ? "text-failed-text" : "text-muted")}>
                {reason}
              </span>
            </>
          )}
        </span>
      ) : (
        <>
          <span aria-hidden className={cx("size-6 shrink-0 rounded-pill motion-reduce:animate-none", dot)} />
          <span role="status" data-state={state} className="type-small font-medium text-fg whitespace-nowrap">
            {WORD[state]}
          </span>
          {waiting ? (
            <DotLoader className="text-faint max-lg:hidden" />
          ) : state === "listening" || state === "speaking" ? (
            <Meter level={level} />
          ) : null}
          <span className="type-mono-xs tabular-nums text-muted" aria-label={`${clock(elapsedSeconds)} elapsed`}>
            {clock(elapsedSeconds)}
          </span>
        </>
      )}
      <span aria-hidden className="h-14 w-px shrink-0 bg-border-strong max-lg:hidden" />
      {over ? (
        <>
          {onRestart && (
            <button
              type="button"
              onClick={onRestart}
              className="h-20 shrink-0 rounded-pill px-6 type-small font-medium text-fg hover:bg-surface focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-focus"
            >
              Start again
            </button>
          )}
          {onDismiss && (
            <button type="button" aria-label="Dismiss" onClick={onDismiss} className={cx(pillButton, "text-muted hover:bg-surface hover:text-fg")}>
              <Icon name="close" size={12} />
            </button>
          )}
        </>
      ) : (
        onEnd && (
          <StopButton onEnd={onEnd} />
        )
      )}
    </div>
  );
}
