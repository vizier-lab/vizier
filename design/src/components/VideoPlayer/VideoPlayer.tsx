import type { HTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type VideoPlayerProps = {
  /** Poster image URL; a plain surface when absent. */
  poster?: string;
  /** "demo-filtered-search.mp4 · 0:18 · 2.4 MB". */
  caption?: ReactNode;
  /** Name for the play button. */
  title: string;
  onPlay: () => void;
} & HTMLAttributes<HTMLElement>;

/** A video in a reply: poster with play, and its caption (X46). Playing swaps in the caller's <video>. */
export function VideoPlayer({ poster, caption, title, onPlay, className, ...rest }: VideoPlayerProps) {
  return (
    <figure className={cx("flex flex-col gap-8", className)} {...rest}>
      <button
        type="button"
        aria-label={`Play ${title}`}
        onClick={onPlay}
        className="relative flex aspect-video w-full items-center justify-center overflow-hidden rounded-lg bg-surface"
      >
        {poster && <img src={poster} alt="" className="absolute inset-0 size-full object-cover" />}
        <span className="relative flex size-40 items-center justify-center rounded-pill bg-ground text-fg">
          <Icon name="play" size={17} />
        </span>
      </button>
      {caption && <figcaption className="type-mono-xs text-faint">{caption}</figcaption>}
    </figure>
  );
}
