import { useEffect, useRef, useState, type HTMLAttributes } from "react";
import { cx } from "../../lib/cx";
import { MARK_ROWS } from "../Mark/geometry";

export type LedBoardProps = {
  /** Live lights the star's four dots emerald. */
  state?: "idle" | "live";
  /** Where the mark sits: high in the panel (Login, Onboarding) or centred. */
  placement?: "upper" | "center";
} & HTMLAttributes<HTMLDivElement>;

// A 22-unit dot field: off dots are 3 wide, lit dots 12.4. Each mark cell is 2×2 dots.
const PITCH = 22;
const OFF_R = 1.5;
const LIT_R = 6.2;
const MARK = MARK_ROWS.length * 2;

/**
 * The LED board (B4): the mark drawn as 2×2 dot cells over a faint dot field.
 * Fills its container; the field is as many dots as fit.
 */
export function LedBoard({ state = "idle", placement = "upper", className, ...rest }: LedBoardProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState({ w: 0, h: 0 });

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const obs = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      setBox({ w: width, h: height });
    });
    obs.observe(el);
    return () => obs.disconnect();
  }, []);

  const cols = Math.max(MARK, Math.floor(box.w / PITCH));
  const rows = Math.max(MARK, Math.floor(box.h / PITCH));
  const left = Math.floor((cols - MARK) / 2);
  const top = placement === "center" ? Math.floor((rows - MARK) / 2) : Math.floor((rows - MARK) / 4);

  const kind = (col: number, row: number): "off" | "lit" | "live" => {
    const x = col - left;
    const y = row - top;
    if (x < 0 || y < 0 || x >= MARK || y >= MARK) return "off";
    const ch = MARK_ROWS[y >> 1][x >> 1];
    if (ch === ".") return "off";
    return ch === "*" && state === "live" ? "live" : "lit";
  };

  const dots: { cx: number; cy: number; k: "off" | "lit" | "live" }[] = [];
  if (box.w > 0) {
    for (let row = 0; row < rows; row++) {
      for (let col = 0; col < cols; col++) {
        dots.push({ cx: col * PITCH + PITCH / 2, cy: row * PITCH + PITCH / 2, k: kind(col, row) });
      }
    }
  }

  return (
    <div ref={ref} role="img" aria-label="Vizier" className={cx("relative size-full overflow-hidden", className)} {...rest}>
      {box.w > 0 && (
        <svg
          width={cols * PITCH}
          height={rows * PITCH}
          className="absolute animate-fade-in motion-reduce:animate-none"
          style={{ left: (box.w - cols * PITCH) / 2, top: (box.h - rows * PITCH) / 2 }}
          aria-hidden
        >
          {dots.map((d) => (
            <circle
              key={`${d.cx}-${d.cy}`}
              cx={d.cx}
              cy={d.cy}
              r={d.k === "off" ? OFF_R : LIT_R}
              className={cx(
                d.k === "off" ? "fill-dot-off" : d.k === "live" ? "fill-live animate-live-glow motion-reduce:animate-none" : "fill-ink",
              )}
            />
          ))}
        </svg>
      )}
    </div>
  );
}
