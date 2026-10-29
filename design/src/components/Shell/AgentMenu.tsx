import { cloneElement, useEffect, useRef, useState, type ReactElement } from "react";
import { motion } from "motion/react";
import { cx } from "../../lib/cx";
import { Presence } from "../../lib/Motion";
import { motionDurations, motionEases } from "../../tokens/tokens";
import { Floating } from "../Floating/Floating";
import { Menu, MenuDivider } from "../Menu/Menu";
import { MenuItem } from "../MenuItem/MenuItem";
import { MENU_GROUPS, type MenuPage } from "./ContextSidebar";

export type AgentMenuProps = {
  /** The SidebarAgent. It gets aria-expanded and the click that opens the menu. */
  trigger: ReactElement<{ onClick?: () => void; "aria-expanded"?: boolean; "aria-haspopup"?: "menu" }>;
  /** The page open, if it is one of the menu's. */
  active?: MenuPage;
  /** Pages the person may not see (Secrets without access). */
  hiddenPages?: MenuPage[];
  /** Shown but not reachable (Config without the right to edit). */
  disabledPages?: MenuPage[];
  onSelect: (page: MenuPage) => void;
  /** The last row, with its shortcut. Omit it when there is nothing to switch to. */
  onSwitchAgent?: () => void;
  switchShortcut?: string;
  /** Controlled, so a shortcut elsewhere can open it. */
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  className?: string;
};

/**
 * The agent's other pages (Core, its connections, usage and settings), opened from the agent card.
 * It scales and fades in from the card's corner and leaves faster than it came.
 */
export function AgentMenu({
  trigger,
  active,
  hiddenPages,
  disabledPages,
  onSelect,
  onSwitchAgent,
  switchShortcut,
  open: openProp,
  onOpenChange,
  className,
}: AgentMenuProps) {
  const [openState, setOpenState] = useState(false);
  const open = openProp ?? openState;
  const setOpen = (v: boolean) => {
    setOpenState(v);
    onOpenChange?.(v);
  };
  const root = useRef<HTMLDivElement>(null);
  const floating = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      const t = e.target as Node;
      if (!root.current?.contains(t) && !floating.current?.contains(t)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("pointerdown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("pointerdown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  });

  const hidden = new Set(hiddenPages);
  const disabled = new Set(disabledPages);
  const groups = MENU_GROUPS.map((g) => ({ ...g, pages: g.pages.filter((p) => !hidden.has(p.page)) })).filter((g) => g.pages.length);

  return (
    <div ref={root} className={cx("w-full", className)}>
      {cloneElement(trigger, { "aria-expanded": open, "aria-haspopup": "menu", onClick: () => setOpen(!open) })}
      <Presence>
        {open && (
          <Floating anchor={root} ref={floating}>
            <motion.div
              style={{ transformOrigin: "top left" }}
              initial={{ opacity: 0, scale: 0.97, y: -4 }}
              animate={{ opacity: 1, scale: 1, y: 0, transition: { duration: motionDurations.base, ease: motionEases.arrive } }}
              exit={{ opacity: 0, scale: 0.98, transition: { duration: motionDurations.fast, ease: motionEases.standard } }}
            >
              <Menu aria-label="Agent pages" className="w-208">
                {groups.map((g, i) => (
                  <div key={g.label} role="group" aria-label={g.label} className="flex flex-col">
                    <div aria-hidden className={cx("px-8 pb-2 type-label uppercase text-faint", i === 0 ? "pt-6" : "pt-10")}>
                      {g.label}
                    </div>
                    {g.pages.map((p) => (
                      <MenuItem
                        key={p.page}
                        icon={p.icon}
                        label={p.label}
                        selected={false}
                        disabled={disabled.has(p.page)}
                        aria-current={active === p.page ? "page" : undefined}
                        className={active === p.page ? "bg-raised text-fg" : undefined}
                        onClick={() => {
                          setOpen(false);
                          onSelect(p.page);
                        }}
                      />
                    ))}
                  </div>
                ))}
                {onSwitchAgent && (
                  <>
                    <MenuDivider />
                    <MenuItem
                      label="Switch agent"
                      meta={switchShortcut}
                      onClick={() => {
                        setOpen(false);
                        onSwitchAgent();
                      }}
                    />
                  </>
                )}
              </Menu>
            </motion.div>
          </Floating>
        )}
      </Presence>
    </div>
  );
}
