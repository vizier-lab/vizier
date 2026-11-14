import { useId, type HTMLAttributes, type ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { Eyebrow } from "../Eyebrow/Eyebrow";

export type SubagentWorkerState = "running" | "asking" | "answered" | "failed" | "timed_out" | "stopped";

export type SubagentWorkerProps = {
  /** The worker's task, as the main agent wrote it. */
  task: string;
  /** Its place in the spawn that hired it: 2 for "2 · …". */
  position: number;
  /** The handle, "s_k3x9a1b2". */
  handle: string;
  state: SubagentWorkerState;
  /** The main agent's name, for "Waiting for Archivist". */
  agentName: string;
  /** "1:12". */
  elapsed?: ReactNode;
  /** The open question, while asking. */
  question?: string;
  /** Whether the main agent has the newest outcome (a final worker only). */
  handedOver?: boolean;
  /** Why it stopped, for the line: "by the agent", "the conversation was deleted". */
  stopReason?: string;
  /** The line under the title. Left out, it is said from the state. */
  summary?: ReactNode;
  expanded?: boolean;
  onExpandedChange?: (expanded: boolean) => void;
  /** The whole task, drawn under TASK when expanded. Defaults to `task`. */
  fullTask?: ReactNode;
  /** The expanded STEPS: the chat's activity trail for this worker. */
  steps?: ReactNode;
  /** How many steps, for the STEPS label. */
  stepCount?: number;
  /** What it answered, under RESULT: plain text keeps its line breaks, or rendered markdown. */
  result?: ReactNode;
  /** Why it failed, under ERROR. */
  error?: ReactNode;
} & Omit<HTMLAttributes<HTMLDivElement>, "children">;

function defaultSummary(p: Pick<SubagentWorkerProps, "state" | "agentName" | "question" | "handedOver" | "stopReason">) {
  switch (p.state) {
    case "running":
      return "Running";
    case "asking":
      return (
        <>
          Waiting for {p.agentName}
          {p.question && <> · “{p.question}”</>}
        </>
      );
    case "stopped":
      return `Stopped${p.stopReason ? ` · ${p.stopReason}` : ""}`;
    default: {
      const word = p.state === "answered" ? "Answered" : p.state === "failed" ? "Failed" : "Ran out of time";
      return `${word} · ${p.handedOver ? `${p.agentName} has it` : `waiting to be delivered to ${p.agentName}`}`;
    }
  }
}

/**
 * One worker in the Subagents panel (spec 059, frame "059 · A"): titled by its position and
 * task, collapsed to a line, and expanded to its task, its steps (the chat's activity trail) and
 * its result. Asking reads amber and quotes the question. There is no stop or reply control:
 * only the main agent manages its workers.
 */
export function SubagentWorker({
  task,
  position,
  handle,
  state,
  agentName,
  elapsed,
  question,
  handedOver,
  stopReason,
  summary,
  expanded = false,
  onExpandedChange,
  fullTask,
  steps,
  stepCount,
  result,
  error,
  className,
  ...rest
}: SubagentWorkerProps) {
  const bodyId = useId();
  const asking = state === "asking";
  const failed = state === "failed" || state === "timed_out";
  return (
    <div
      className={cx(
        "flex flex-col rounded-md border bg-surface",
        asking ? "border-retrying" : expanded ? "border-live" : "border-border",
        className,
      )}
      {...rest}
    >
      <button
        type="button"
        aria-expanded={expanded}
        aria-controls={expanded ? bodyId : undefined}
        onClick={() => onExpandedChange?.(!expanded)}
        className="flex flex-col gap-4 rounded-md px-12 py-10 text-left hover:bg-raised focus-visible:outline-2 focus-visible:outline-focus"
      >
        <span className="flex items-center gap-8">
          <span
            aria-hidden
            className={cx(
              "size-7 shrink-0 rounded-pill",
              state === "running" ? "bg-live" : asking ? "bg-retrying" : failed ? "bg-failed" : "bg-faint",
            )}
          />
          <span className="min-w-0 flex-1 truncate type-ui text-fg">
            {position} · {task}
          </span>
          <span className="shrink-0 type-mono-sm text-faint">
            {handle}
            {elapsed != null && <> · {elapsed}</>}
          </span>
          <Icon name={expanded ? "chevron-down" : "chevron-right"} size={14} className="shrink-0 text-soft" />
        </span>
        <span
          className={cx("type-small", asking ? "text-retrying" : failed ? "text-failed" : "text-faint", asking && !expanded && "line-clamp-2")}
        >
          {summary ?? defaultSummary({ state, agentName, question, handedOver, stopReason })}
        </span>
      </button>
      {expanded && (
        <div id={bodyId} className="flex flex-col gap-12 px-12 pb-12">
          <section className="flex flex-col gap-6">
            <Eyebrow as="h4">Task</Eyebrow>
            <p className="whitespace-pre-wrap type-body text-fg">{fullTask ?? task}</p>
          </section>
          {steps && (
            <section className="flex flex-col gap-6">
              <Eyebrow as="h4" count={stepCount}>
                Steps
              </Eyebrow>
              {steps}
            </section>
          )}
          {result != null && (
            <section className="flex flex-col gap-6">
              <Eyebrow as="h4">Result</Eyebrow>
              <div
                className={cx(
                  "rounded-md border border-border bg-panel px-16 py-12 type-body text-fg",
                  typeof result === "string" && "whitespace-pre-wrap",
                )}
              >
                {result}
              </div>
            </section>
          )}
          {error != null && (
            <section className="flex flex-col gap-6">
              <Eyebrow as="h4">Error</Eyebrow>
              <p className="whitespace-pre-wrap type-small text-failed">{error}</p>
            </section>
          )}
        </div>
      )}
    </div>
  );
}
