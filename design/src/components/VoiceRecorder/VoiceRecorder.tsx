import type { HTMLAttributes } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { Waveform, sampleLevels } from "../AudioPlayer/Waveform";

export type VoiceRecorderProps = {
  /** Recording: a live timer and Stop. Recorded: play it back, or Discard. */
  mode: "recording" | "recorded";
  /** Seconds recorded. */
  elapsed: number;
  levels?: number[];
  onStop?: () => void;
  onDiscard?: () => void;
  playing?: boolean;
  onPlayingChange?: (playing: boolean) => void;
  /** Recorded only: how far playback has got, 0–1. */
  progress?: number;
} & HTMLAttributes<HTMLDivElement>;

const clock = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`;

/** The composer's voice row (X35): record, then listen back before sending. */
export function VoiceRecorder({ mode, elapsed, levels = sampleLevels(24, 3), onStop, onDiscard, playing, onPlayingChange, progress = 0, className, ...rest }: VoiceRecorderProps) {
  if (mode === "recording") {
    return (
      <div role="status" className={cx("flex items-center gap-10", className)} {...rest}>
        <span aria-hidden className="size-8 rounded-pill bg-failed animate-shimmer motion-reduce:animate-none" />
        <span className="type-mono-sm-medium text-fg" aria-label={`Recording, ${clock(elapsed)}`}>
          {clock(elapsed)}
        </span>
        <Waveform levels={levels} live className="min-w-0 flex-1 overflow-hidden" />
        <button type="button" onClick={onStop} className="rounded-xs type-ui-medium text-fg hover:underline">
          Stop
        </button>
      </div>
    );
  }
  return (
    <div className={cx("flex items-center gap-10", className)} {...rest}>
      <button
        type="button"
        aria-label={playing ? "Pause recording" : "Play recording"}
        onClick={() => onPlayingChange?.(!playing)}
        className="flex size-24 shrink-0 items-center justify-center rounded-pill border border-border-strong text-fg hover:bg-raised"
      >
        <Icon name={playing ? "square" : "play"} size={11} />
      </button>
      <span aria-hidden className="relative h-2 min-w-0 flex-1 rounded-pill bg-border-strong">
        <span className="absolute inset-y-0 left-0 rounded-pill bg-fg" style={{ width: `${progress * 100}%` }} />
      </span>
      <span className="type-mono-xs text-faint">{clock(elapsed)}</span>
      <button type="button" onClick={onDiscard} className="rounded-xs type-ui-medium text-fg hover:underline">
        Discard
      </button>
    </div>
  );
}
