import type { HTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons/map";
import { cx } from "../../lib/cx";
import { ListItem } from "../ListItem/ListItem";
import { ESSENTIALS, MENU_GROUPS, type EssentialPage, type MenuPage } from "../Shell/ContextSidebar";

export type AgentDetailsPage = EssentialPage | MenuPage;

type Row = { page: AgentDetailsPage; label: string; icon: IconName };

const essential = (page: EssentialPage) => ESSENTIALS.find((p) => p.page === page)!;

/**
 * The groups of the phone's Agent details page, from the same page lists as the desktop's
 * agent column and menu, so a page added there shows here too.
 */
export const AGENT_DETAILS_GROUPS: { label: string; rows: Row[] }[] = [
  { label: "The agent", rows: [essential("memory")] },
  { label: "Essentials", rows: ESSENTIALS.filter((p) => p.page !== "memory") },
  ...MENU_GROUPS.map((g) => ({ label: g.label, rows: g.pages })),
];

export type AgentDetailsListProps = {
  hrefFor: (page: AgentDetailsPage) => string;
  /** Pages the person may not see (Secrets without access, Artifacts without view). */
  hiddenPages?: readonly AgentDetailsPage[];
  /** Shown but not reachable (Config without the right to edit). */
  disabledPages?: readonly AgentDetailsPage[];
  /** A short value at the end of a row: "5", "2 open". Only what is already loaded. */
  counts?: Partial<Record<AgentDetailsPage, ReactNode>>;
} & HTMLAttributes<HTMLDivElement>;

/** Everything about one agent, as grouped rows (spec 056): the phone's way into the agent's pages. */
export function AgentDetailsList({ hrefFor, hiddenPages, disabledPages, counts, className, ...rest }: AgentDetailsListProps) {
  const groups = AGENT_DETAILS_GROUPS.map((g) => ({ ...g, rows: g.rows.filter((r) => !hiddenPages?.includes(r.page)) })).filter(
    (g) => g.rows.length,
  );
  return (
    <div className={cx("flex flex-col gap-20", className)} {...rest}>
      {groups.map((g) => (
        <section key={g.label} aria-label={g.label} className="flex flex-col gap-8">
          <h2 className="px-4 type-label uppercase text-faint">{g.label}</h2>
          <div className="flex flex-col divide-y divide-border overflow-hidden rounded-xl bg-surface">
            {g.rows.map((r) => (
              <ListItem
                key={r.page}
                shape="flush"
                className="min-h-48"
                leading={<Icon name={r.icon} size={16} tone="muted" />}
                title={<span className="type-body text-fg">{r.label}</span>}
                meta={counts?.[r.page]}
                trailing={<Icon name="chevron-right" size={15} tone="faint" />}
                href={hrefFor(r.page)}
                disabled={disabledPages?.includes(r.page)}
              />
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}
