import { useEffect, useRef, type RefObject } from "react";
import { motion } from "motion/react";
import { useDismiss } from "../../lib/dismiss";
import { motionDurations, motionEases } from "../../tokens/tokens";
import { Floating } from "../Floating/Floating";
import { Menu, MenuDivider } from "../Menu/Menu";
import { MenuItem } from "../MenuItem/MenuItem";
import { AgentMenuGroups } from "./AgentMenu";
import { ESSENTIALS, type EssentialPage, type MenuPage } from "./ContextSidebar";

export type RailAgentPage = EssentialPage | MenuPage;

export type RailAgentMenuProps = {
  /** The agent's tile. */
  anchor: RefObject<HTMLElement | null>;
  name: string;
  /** The page open, when this agent's pages are the ones showing. */
  active?: RailAgentPage;
  hiddenPages?: readonly RailAgentPage[];
  disabledPages?: readonly RailAgentPage[];
  onNew: () => void;
  onSelect: (page: RailAgentPage) => void;
  onClose: () => void;
};

/**
 * What a right click on a rail tile opens: everything the agent's column and card menu reach,
 * without its conversations and projects.
 */
export function RailAgentMenu({ anchor, name, active, hiddenPages, disabledPages, onNew, onSelect, onClose }: RailAgentMenuProps) {
  const panel = useRef<HTMLDivElement>(null);
  useDismiss(true, onClose, panel);
  // A context-menu key opens this too, so the arrow keys have to work from the start. It waits a
  // frame because Floating stays hidden until it is placed, and a hidden item can't take focus.
  useEffect(() => {
    const frame = requestAnimationFrame(() => panel.current?.querySelector<HTMLElement>('[role^="menuitem"]:not(:disabled)')?.focus());
    return () => cancelAnimationFrame(frame);
  }, []);

  const choose = (act: () => void) => () => {
    onClose();
    act();
  };
  const activeEssential = (page: EssentialPage) => (active === page ? "bg-raised text-fg" : undefined);

  return (
    <Floating anchor={anchor} side="right" gap={8} ref={panel} className="overflow-y-auto">
      <motion.div
        style={{ transformOrigin: "top left" }}
        initial={{ opacity: 0, scale: 0.97, x: -4 }}
        animate={{ opacity: 1, scale: 1, x: 0, transition: { duration: motionDurations.base, ease: motionEases.arrive } }}
        exit={{ opacity: 0, scale: 0.98, transition: { duration: motionDurations.fast, ease: motionEases.standard } }}
      >
        <Menu
          aria-label={`${name} pages`}
          onKeyDown={(e) => {
            // Back to the tile, so a keyboard user carries on from where they were.
            if (e.key === "Escape") anchor.current?.focus();
          }}
        >
          <MenuItem icon="chat-plus" label="New" selected={false} onClick={choose(onNew)} />
          {ESSENTIALS.filter((p) => !hiddenPages?.includes(p.page)).map((p) => (
            <MenuItem
              key={p.page}
              icon={p.icon}
              label={p.label}
              selected={false}
              disabled={disabledPages?.includes(p.page)}
              aria-current={active === p.page ? "page" : undefined}
              className={activeEssential(p.page)}
              onClick={choose(() => onSelect(p.page))}
            />
          ))}
          <MenuDivider />
          <AgentMenuGroups
            active={active !== undefined && !ESSENTIALS.some((p) => p.page === active) ? (active as MenuPage) : undefined}
            hiddenPages={hiddenPages}
            disabledPages={disabledPages}
            onSelect={(page) => choose(() => onSelect(page))()}
          />
        </Menu>
      </motion.div>
    </Floating>
  );
}
