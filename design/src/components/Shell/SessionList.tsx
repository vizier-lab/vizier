import type { ReactNode } from "react";
import { AnimatePresence, LayoutGroup, motion } from "motion/react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { motionDurations, motionEases } from "../../tokens/tokens";

export type SessionItem = {
  key: string;
  title: string;
  /** "now", "2h", "18:40", "Tue". */
  time: string;
  /** A turn is running in it. */
  live?: boolean;
  href: string;
  active?: boolean;
};

export type SessionGroup = { label: string; items: SessionItem[] };

export type SessionListProps = {
  groups: SessionGroup[];
  /** Starts a new conversation. */
  onNew?: () => void;
  newLabel?: string;
  /** The "Show all 63" row; omit it when the list is whole. */
  showAll?: { label: ReactNode; onSelect: () => void };
  className?: string;
};

const EASE = { duration: motionDurations.base, ease: motionEases.standard };

/**
 * The agent's conversations in the context sidebar: newest first, in Today / Yesterday / This week
 * groups. A new one slides in at the top, a removed one folds away, and the lit row slides to the
 * conversation you open.
 */
export function SessionList({ groups, onNew, newLabel = "New session", showAll, className }: SessionListProps) {
  return (
    <section aria-label="Sessions" className={cx("flex min-h-0 flex-1 flex-col pt-8", className)}>
      <header className="flex h-24 shrink-0 items-center justify-between px-10">
        <h2 className="type-ui-medium text-faint">Sessions</h2>
        {onNew && (
          <button
            type="button"
            aria-label={newLabel}
            title={newLabel}
            onClick={onNew}
            className="grid size-24 place-items-center rounded-md bg-surface text-muted transition-colors hover:bg-raised hover:text-fg"
          >
            <Icon name="plus" size={12} />
          </button>
        )}
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto pt-4 [scrollbar-width:none]">
        <LayoutGroup>
          {groups.map((g) => (
            <div key={g.label} role="group" aria-label={g.label} className="flex flex-col pb-8">
              <div aria-hidden className="px-10 pt-8 pb-4 type-mono-xs text-faint">
                {g.label}
              </div>
              <ul className="m-0 flex list-none flex-col p-0">
                <AnimatePresence initial={false}>
                  {g.items.map((s) => (
                    <motion.li
                      key={s.key}
                      layout="position"
                      initial={{ opacity: 0, height: 0 }}
                      animate={{ opacity: 1, height: 30, transition: EASE }}
                      exit={{ opacity: 0, height: 0, transition: { ...EASE, duration: motionDurations.fast } }}
                      className="relative overflow-hidden"
                    >
                      <a
                        href={s.href}
                        aria-current={s.active ? "page" : undefined}
                        className={cx(
                          "relative flex h-30 items-center gap-8 rounded-lg px-10 transition-colors duration-[var(--vz-duration-base)]",
                          s.active ? "text-fg" : "text-muted hover:bg-surface/60 hover:text-soft",
                        )}
                      >
                        {s.active && (
                          <motion.span
                            layoutId="session-lit"
                            aria-hidden
                            className="absolute inset-0 rounded-lg bg-surface"
                            transition={EASE}
                          />
                        )}
                        <span className={cx("relative min-w-0 flex-1 truncate type-body", s.active && "type-body-medium")}>{s.title}</span>
                        {s.live && <span role="img" aria-label="a turn is running" className="relative size-6 shrink-0 rounded-pill bg-live" />}
                        <span className={cx("relative shrink-0 type-mono-xs", s.live ? "text-live-text" : "text-faint")}>{s.time}</span>
                      </a>
                    </motion.li>
                  ))}
                </AnimatePresence>
              </ul>
            </div>
          ))}
        </LayoutGroup>
        {showAll && (
          <button
            type="button"
            onClick={showAll.onSelect}
            className="px-10 py-8 text-left type-small text-faint transition-colors hover:text-fg"
          >
            {showAll.label}
          </button>
        )}
      </div>
    </section>
  );
}
