import { useEffect, useRef, type ReactNode } from "react";
import { Button } from "../Button/Button";
import { DotLoader } from "../DotLoader/DotLoader";
import { cx } from "../../lib/cx";

export type LoadMoreProps = {
  onLoad: () => void;
  loading?: boolean;
  /** "Load older", "Show all 12". */
  label: ReactNode;
  /** How many are still hidden, shown after the label. */
  remaining?: number;
  /** `auto` loads when scrolled into view; `button` waits for a click. */
  look?: "button" | "auto";
  className?: string;
};

/**
 * The end of a paged list (X81): feeds, histories, side panels and the chat's
 * upward history load. Never fetches by itself; `onLoad` does.
 */
export function LoadMore({ onLoad, loading, label, remaining, look = "button", className }: LoadMoreProps) {
  const sentinel = useRef<HTMLDivElement>(null);
  const load = useRef(onLoad);
  load.current = onLoad;

  useEffect(() => {
    if (look !== "auto" || loading) return;
    const el = sentinel.current;
    if (!el || typeof IntersectionObserver === "undefined") return;
    const io = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) load.current();
    });
    io.observe(el);
    return () => io.disconnect();
  }, [look, loading]);

  const text = (
    <>
      {label}
      {remaining != null && <span className="type-mono-xs text-faint"> · {remaining.toLocaleString("en-US")} more</span>}
    </>
  );

  return (
    <div ref={sentinel} className={cx("flex justify-center py-12", className)} aria-live="polite">
      {loading ? (
        <span className="inline-flex items-center gap-8 type-small text-muted">
          <DotLoader /> Loading
        </span>
      ) : look === "auto" ? (
        <span className="type-small text-faint">{text}</span>
      ) : (
        <Button variant="ghost" onClick={onLoad}>
          {text}
        </Button>
      )}
    </div>
  );
}
