import { useState, type HTMLAttributes, type ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { DotLoader } from "../DotLoader/DotLoader";
import { TextLink } from "../TextLink/TextLink";
import { Waveform, sampleLevels } from "./Waveform";

export type AudioPlayerProps = {
  playing: boolean;
  onPlayingChange: (playing: boolean) => void;
  /** Seconds. */
  position: number;
  duration: number;
  levels?: number[];
  speed?: number;
  onSpeedChange?: (speed: number) => void;
  /** The spoken text; "Show transcript" reveals it. */
  transcript?: ReactNode;
  /** The transcript is on its way (a live voice message just heard): "Transcribing" holds its place. */
  transcribing?: boolean;
  /** The recording is not here yet (a live voice reply still being spoken): the player waits, faint,
   * with "Speaking" where the time goes; the transcript is already readable. */
  pending?: boolean;
} & HTMLAttributes<HTMLDivElement>;

const clock = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`;
const SPEEDS = [1, 1.5, 2];

/** A spoken reply that plays in place (X45). The caller owns the <audio> element. */
export function AudioPlayer({
  playing,
  onPlayingChange,
  position,
  duration,
  levels = sampleLevels(48),
  speed = 1,
  onSpeedChange,
  transcript,
  transcribing,
  pending,
  className,
  ...rest
}: AudioPlayerProps) {
  const [showText, setShowText] = useState(false);
  return (
    <div className={cx("flex flex-col gap-8 rounded-lg border border-border bg-surface px-12 py-10", className)} {...rest}>
      <div className="flex items-center gap-12">
        <button
          type="button"
          aria-label={playing ? "Pause" : "Play"}
          onClick={() => onPlayingChange(!playing)}
          disabled={pending}
          className="flex size-28 shrink-0 items-center justify-center rounded-pill bg-ink text-ground enabled:hover:bg-ink-hover disabled:opacity-40"
        >
          <Icon name={playing ? "square" : "play"} size={13} />
        </button>
        <Waveform
          levels={levels}
          progress={duration ? position / duration : 0}
          className={cx(pending && "opacity-40")}
        />
        {pending ? (
          <span role="status" className="flex shrink-0 items-center gap-4 type-small text-muted">
            Speaking
            <DotLoader />
          </span>
        ) : (
          <span className="shrink-0 type-mono-xs text-faint">
            {clock(position)} / {clock(duration)}
          </span>
        )}
        {!pending && <button
          type="button"
          aria-label={`Speed ${speed}×`}
          onClick={() => onSpeedChange?.(SPEEDS[(SPEEDS.indexOf(speed) + 1) % SPEEDS.length])}
          className="shrink-0 rounded-xs type-mono-xs text-muted hover:text-fg"
        >
          {speed}×
        </button>}
      </div>
      {!transcript && transcribing && (
        <p role="status" className="flex items-center gap-4 type-small text-faint">
          Transcribing
          <DotLoader />
        </p>
      )}
      {transcript && (
        <>
          <div>
            <TextLink size="ui" onClick={() => setShowText(!showText)} aria-expanded={showText}>
              {showText ? "Hide transcript" : "Show transcript"}
            </TextLink>
          </div>
          {showText && <p className="type-ui-para text-muted">{transcript}</p>}
        </>
      )}
    </div>
  );
}
