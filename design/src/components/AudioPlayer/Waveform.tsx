import { cx } from "../../lib/cx";

/** Bars for an audio level series (0–1); bars before `progress` (0–1) are lit. */
export function Waveform({ levels, progress = 1, live, className }: { levels: number[]; progress?: number; live?: boolean; className?: string }) {
  return (
    <span aria-hidden className={cx("flex h-18 items-center gap-2", className)}>
      {levels.map((l, i) => (
        <span
          key={i}
          className={cx("w-2 shrink-0 rounded-pill", i / levels.length < progress ? (live ? "bg-fg" : "bg-soft") : "bg-unlit")}
          style={{ height: `${Math.max(12, Math.round(l * 100))}%` }}
        />
      ))}
    </span>
  );
}

/** A deterministic level series for stories and placeholders. */
export function sampleLevels(n: number, seed = 7) {
  let x = seed;
  return Array.from({ length: n }, () => {
    x = (x * 9301 + 49297) % 233280;
    return 0.2 + (x / 233280) * 0.8;
  });
}
