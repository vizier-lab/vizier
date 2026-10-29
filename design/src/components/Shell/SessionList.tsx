import { useEffect, useRef, useState, type ReactNode } from "react";
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
  /** Shows a delete button on hover and focus. */
  deletable?: boolean;
};

export type SessionGroup = { label: string; items: SessionItem[] };

export type SessionListProps = {
  groups: SessionGroup[];
  /** Starts a new conversation. */
  onNew?: () => void;
  newLabel?: string;
  /** Called with the item's key when its delete button is pressed. The caller confirms and removes it. */
  onDelete?: (key: string) => void;
  /** A search over the conversations. The field opens from the header and stays open while it holds text. */
  search?: { value: string; onChange: (value: string) => void; placeholder?: string };
  /** What to say when the list is empty; the search's "nothing matched" goes here. */
  empty?: ReactNode;
  /** The "Show more" row; omit it when the list is whole. */
  showAll?: { label: ReactNode; onSelect: () => void };
  className?: string;
};

const EASE = { duration: motionDurations.base, ease: motionEases.standard };

/**
 * The agent's conversations in the context sidebar: newest first, in Today / Yesterday / This week
 * groups. A new one slides in at the top, a removed one folds away, and the lit row slides to the
 * conversation you open.
 */
export function SessionList({ groups, onNew, newLabel = "New session", onDelete, search, empty, showAll, className }: SessionListProps) {
  const [opened, setOpened] = useState(false);
  const open = opened || !!search?.value;
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (opened) input.current?.focus();
  }, [opened]);
  const close = () => {
    search?.onChange("");
    setOpened(false);
  };
  return (
    <section aria-label="Sessions" className={cx("flex min-h-0 flex-1 flex-col pt-8", className)}>
      <header className="flex h-24 shrink-0 items-center justify-between px-10">
        <h2 className="type-ui-medium text-faint">Sessions</h2>
        <span className="flex items-center gap-4">
        {search && (
          <button
            type="button"
            aria-label="Search sessions"
            title="Search sessions"
            aria-pressed={open}
            onClick={() => (open ? close() : setOpened(true))}
            className={cx(
              "grid size-24 place-items-center rounded-md transition-colors hover:bg-raised hover:text-fg",
              open ? "bg-raised text-fg" : "bg-surface text-muted",
            )}
          >
            <Icon name="search" size={12} />
          </button>
        )}
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
        </span>
      </header>
      <AnimatePresence initial={false}>
        {search && open && (
          <motion.div
            key="search"
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto", transition: EASE }}
            exit={{ opacity: 0, height: 0, transition: { ...EASE, duration: motionDurations.fast } }}
            className="shrink-0 overflow-hidden"
          >
            <label className="mt-8 flex h-30 items-center gap-8 rounded-lg border border-border bg-surface px-10 focus-within:border-border-hover">
              <Icon name="search" size={13} tone="faint" />
              <input
                ref={input}
                value={search.value}
                onChange={(e) => search.onChange(e.target.value)}
                onKeyDown={(e) => e.key === "Escape" && close()}
                placeholder={search.placeholder ?? "Search sessions"}
                aria-label="Search sessions"
                className="min-w-0 flex-1 bg-transparent type-small text-fg outline-none placeholder:text-faint"
              />
            </label>
          </motion.div>
        )}
      </AnimatePresence>
      <div className="min-h-0 flex-1 overflow-y-auto pt-4 [scrollbar-width:none]">
        {groups.length === 0 && empty && <div className="px-10 py-12 type-small text-faint">{empty}</div>}
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
                      className="group/row relative overflow-hidden"
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
                        <span className={cx("relative min-w-0 flex-1 truncate type-ui", s.active && "type-ui-medium")}>{s.title}</span>
                        {s.live && <span role="img" aria-label="a turn is running" className="relative size-6 shrink-0 rounded-pill bg-live" />}
                        <span
                          className={cx(
                            "relative shrink-0 type-mono-xs transition-opacity",
                            s.live ? "text-live-text" : "text-faint",
                            onDelete && s.deletable && "group-hover/row:opacity-0 group-focus-within/row:opacity-0",
                          )}
                        >
                          {s.time}
                        </span>
                      </a>
                      {onDelete && s.deletable && (
                        <button
                          type="button"
                          aria-label={`Delete ${s.title}`}
                          title="Delete conversation"
                          onClick={() => onDelete(s.key)}
                          className="absolute top-1/2 right-6 grid size-24 -translate-y-1/2 place-items-center rounded-md text-faint opacity-0 transition-[opacity,color,background-color] hover:bg-raised hover:text-failed focus-visible:opacity-100 group-hover/row:opacity-100 group-focus-within/row:opacity-100"
                        >
                          <Icon name="trash" size={13} />
                        </button>
                      )}
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
