import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";

export type StatusState =
  | "live" | "idle" | "failed" | "retrying" | "starting" | "stopped"
  | "answered" | "running" | "no-response" | "not-run" | "interrupted"
  | "connected" | "reconnecting" | "disabled"
  | "reading" | "installed";

/** An agent's states: live is up and waiting, running has a turn in progress. */
export type AgentState = Extract<StatusState, "live" | "running" | "failed" | "retrying" | "starting" | "stopped">;

/** Agents speak lowercase ("running"), unlike the task table's "Running". */
export const agentStatusLabel = (state: AgentState) => (state === "running" ? "running" : undefined);

export type StatusProps = {
  state: StatusState;
  /** Defaults to the state's own words. */
  label?: ReactNode;
  /** After a middle dot: "retry in 40s", "timed out listing tools". */
  detail?: ReactNode;
  /** Mono (agents, the design-system default), small mono (row subtitles), or sans (task and integration tables). */
  font?: "mono" | "mono-xs" | "sans";
  /** Words only, coloured by state (the agent switcher's status line). */
  dot?: boolean;
} & HTMLAttributes<HTMLSpanElement>;

type Dot = "filled" | "ring" | "halo" | "hollow" | "dashed" | "spin";

/**
 * Every state, in one table: its dot shape, colours and words. A new state is
 * added here and to the Status story, nowhere else.
 */
const STATES: Record<StatusState, { dot: Dot; dotColour: string; text: string; words: string }> = {
  live: { dot: "filled", dotColour: "bg-live", text: "text-live-text", words: "live" },
  idle: { dot: "hollow", dotColour: "border-faint", text: "text-faint", words: "idle" },
  failed: { dot: "filled", dotColour: "bg-failed", text: "text-failed", words: "failed" },
  retrying: { dot: "filled", dotColour: "bg-retrying", text: "text-retrying", words: "retrying" },
  starting: { dot: "ring", dotColour: "border-muted bg-unlit", text: "text-muted", words: "starting" },
  stopped: { dot: "ring", dotColour: "border-unlit bg-dot-off", text: "text-faint", words: "stopped" },
  answered: { dot: "filled", dotColour: "bg-ink", text: "text-fg", words: "Answered" },
  running: { dot: "halo", dotColour: "bg-live", text: "text-live-text", words: "Running" },
  "no-response": { dot: "hollow", dotColour: "border-retrying", text: "text-retrying", words: "No response" },
  "not-run": { dot: "hollow", dotColour: "border-faint", text: "text-faint", words: "Not run yet" },
  interrupted: { dot: "dashed", dotColour: "border-faint", text: "text-faint", words: "Interrupted" },
  connected: { dot: "filled", dotColour: "bg-ink", text: "text-fg", words: "Connected" },
  reconnecting: { dot: "spin", dotColour: "border-retrying", text: "text-retrying", words: "Reconnecting" },
  disabled: { dot: "hollow", dotColour: "border-faint", text: "text-muted", words: "Disabled" },
  reading: { dot: "halo", dotColour: "bg-live", text: "text-live-text", words: "Reading" },
  installed: { dot: "filled", dotColour: "bg-ink", text: "text-muted", words: "Installed" },
};

function DotMark({ dot, colour }: { dot: Dot; colour: string }) {
  const base = "size-7 rounded-pill";
  return (
    <span aria-hidden className="relative flex size-12 shrink-0 items-center justify-center">
      {dot === "halo" && <span className={cx("absolute inset-0 rounded-pill border border-live opacity-50")} />}
      <span
        className={cx(
          base,
          colour,
          dot === "ring" || dot === "hollow" ? "border" : "",
          dot === "dashed" && "border border-dashed",
          dot === "spin" && "animate-spin border border-dashed motion-reduce:animate-none",
        )}
      />
    </span>
  );
}

/** Just the dot of a state, for chips and rows that carry their words elsewhere. */
export function StatusDot({ state }: { state: StatusState }) {
  const s = STATES[state];
  return <DotMark dot={s.dot} colour={s.dotColour} />;
}

/** A state dot with its words (D24, widened to task runs, MCP servers and integrations). */
export function Status({ state, label, detail, font = "mono", dot = true, className, ...rest }: StatusProps) {
  const s = STATES[state];
  return (
    <span
      className={cx("inline-flex items-center gap-6", font === "mono" ? "type-mono-sm" : font === "mono-xs" ? "type-mono-xs" : "type-ui", s.text, className)}
      {...rest}
    >
      {dot && <DotMark dot={s.dot} colour={s.dotColour} />}
      <span className="whitespace-nowrap">
        {label ?? s.words}
        {detail != null && <> · {detail}</>}
      </span>
    </span>
  );
}

export const statusStates = Object.keys(STATES) as StatusState[];
