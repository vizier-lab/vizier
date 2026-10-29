import type { ReactNode } from "react";
import { motion } from "motion/react";
import { Presence } from "../../lib/Motion";
import { motionDurations, motionEases } from "../../tokens/tokens";

export type ShellFrameProps = {
  rail: ReactNode;
  /** The context column beside the rail; none on Home and New agent. */
  sidebar?: ReactNode;
  /** Changes when the column's kind changes (agent pages → settings), so its content crossfades. */
  sidebarKey?: string;
  children: ReactNode;
};

const SIDEBAR = 208;

/**
 * The app frame: the rail and the context sidebar sit on the shell, and the page is a panel inset
 * from the shell's edge. The column slides open and shut when you move between a page that has one
 * and a page that does not, and crossfades when it changes what it shows.
 */
export function ShellFrame({ rail, sidebar, sidebarKey = "sidebar", children }: ShellFrameProps) {
  return (
    <div className="flex h-dvh bg-shell text-fg">
      {rail}
      <Presence>
        {sidebar && (
          <motion.div
            key="sidebar-slot"
            className="shrink-0 overflow-hidden"
            initial={{ width: 0, opacity: 0 }}
            animate={{ width: SIDEBAR, opacity: 1, transition: { duration: motionDurations.slow, ease: motionEases.arrive } }}
            exit={{ width: 0, opacity: 0, transition: { duration: motionDurations.base, ease: motionEases.standard } }}
          >
            <motion.div
              key={sidebarKey}
              className="h-full w-208"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1, transition: { duration: motionDurations.base } }}
            >
              {sidebar}
            </motion.div>
          </motion.div>
        )}
      </Presence>
      <main className="my-12 mr-12 ml-12 flex min-w-0 flex-1 flex-col overflow-hidden rounded-2xl border border-border bg-panel">{children}</main>
    </div>
  );
}
