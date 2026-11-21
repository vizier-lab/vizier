import { useState, type HTMLAttributes, type ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
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
          className="flex size-28 shrink-0 items-center justify-center rounded-pill bg-ink text-ground hover:bg-ink-hover"
        >
          <Icon name={playing ? "square" : "play"} size={13} />
        </button>
        <Waveform levels={levels} progress={duration ? position / duration : 0} className="min-w-0 flex-1 overflow-hidden" />
        <span className="shrink-0 type-mono-xs text-faint">
          {clock(position)} / {clock(duration)}
        </span>
        <button
          type="button"
          aria-label={`Speed ${speed}×`}
          onClick={() => onSpeedChange?.(SPEEDS[(SPEEDS.indexOf(speed) + 1) % SPEEDS.length])}
          className="shrink-0 rounded-xs type-mono-xs text-muted hover:text-fg"
        >
          {speed}×
        </button>
      </div>
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
