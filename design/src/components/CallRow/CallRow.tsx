import type { HTMLAttributes } from "react";
import { Icon } from "../../icons/Icon";
import { DotLoader } from "../DotLoader/DotLoader";
import { Waveform } from "../AudioPlayer/Waveform";
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

export type CallRowProps = {
  state: CallState;
  /** Seconds since the call started, shown as m:ss. */
  elapsedSeconds?: number;
  /**
   * The sound as it is heard, 0 to 1, newest last: the microphone while listening, the reply while
   * speaking. Drawn like the recorder's waveform, scrolling left as levels arrive.
   */
  levels?: number[];
  /** Why the call ended or failed, in a few words: "microphone blocked". */
  reason?: string;
  /** Ends the call. Offered while it runs. */
  onEnd?: () => void;
  /** Starts a new call after an ending. */
  onRestart?: () => void;
  /** Puts the ended row away. */
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

function clock(total: number): string {
  const s = Math.max(0, Math.floor(total));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

const textButton = "shrink-0 rounded-xs type-ui-medium hover:underline focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-focus";

/**
 * A live voice call, as a row in the composer above the text (spec 055): drawn like the voice
 * recorder's row, so a call and a recording read as one family and the composer deck's lip keeps
 * only the strip. While the call runs: a dot, the state word, the time, the sound as a waveform
 * filling the row, and End. After it ends: the reason, Start again and a close. What the agent does
 * meanwhile shows in its activity trail, never here.
 */
export function CallRow({
  state,
  elapsedSeconds = 0,
  levels = [],
  reason,
  onEnd,
  onRestart,
  onDismiss,
  className,
  ...rest
}: CallRowProps) {
  const over = state === "ended" || state === "failed";
  const waiting = state === "connecting";
  const sounding = state === "listening" || state === "speaking";

  if (over) {
    return (
      <div className={cx("flex min-h-24 items-center gap-10", className)} {...rest}>
        <span role="status" data-state={state} className="flex min-w-0 flex-1 items-center gap-6 type-ui whitespace-nowrap">
          <span className="font-medium text-fg">{WORD[state]}</span>
          {reason && (
            <>
              <span aria-hidden className="text-faint">·</span>
              <span title={reason} className={cx("min-w-0 truncate", state === "failed" ? "text-failed-text" : "text-muted")}>
                {reason}
              </span>
            </>
          )}
        </span>
        {onRestart && (
          <button type="button" onClick={onRestart} className={cx(textButton, "text-fg")}>
            Start again
          </button>
        )}
        {onDismiss && (
          <button
            type="button"
            aria-label="Dismiss"
            onClick={onDismiss}
            className="flex size-24 shrink-0 items-center justify-center rounded-pill text-muted hover:bg-raised hover:text-fg focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-focus"
          >
            <Icon name="close" size={12} />
          </button>
        )}
      </div>
    );
  }

  const dot = state === "reconnecting" ? "bg-retrying animate-shimmer" : waiting ? "bg-live animate-shimmer" : "bg-live";
  return (
    <div className={cx("flex min-h-24 items-center gap-10", className)} {...rest}>
      <span aria-hidden className={cx("size-8 shrink-0 rounded-pill motion-reduce:animate-none", dot)} />
      <span role="status" data-state={state} className="shrink-0 type-ui-medium text-fg whitespace-nowrap">
        {WORD[state]}
      </span>
      <span className="shrink-0 type-mono-sm-medium tabular-nums text-muted" aria-label={`${clock(elapsedSeconds)} elapsed`}>
        {clock(elapsedSeconds)}
      </span>
      {sounding ? (
        <Waveform levels={levels} live />
      ) : (
        <span className="flex min-w-0 flex-1 items-center">{waiting && <DotLoader className="text-faint" />}</span>
      )}
      {onEnd && (
        <button type="button" aria-label="End live voice" onClick={onEnd} className={cx(textButton, "text-failed")}>
          End
        </button>
      )}
    </div>
  );
}
