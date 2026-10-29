import { cx } from "../../lib/cx";
import type { AgentState } from "../Status/Status";

/** The dot's colour: live and running are emerald, a failing agent amber or red, the rest grey. */
const DOT: Record<AgentState, string> = {
  live: "bg-live",
  running: "bg-live",
  starting: "bg-muted",
  retrying: "bg-retrying",
  failed: "bg-failed",
  stopped: "bg-unlit",
};

/** An agent's state as a dot in the bottom-right corner of its icon, which must be `relative`. */
export function StatusDot({ state, className }: { state: AgentState; className?: string }) {
  return <span aria-hidden className={cx("absolute -right-3 -bottom-3 size-10 rounded-pill border-2 border-shell", DOT[state], className)} />;
}
