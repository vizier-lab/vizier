import { useId, type ReactNode } from "react";
import { motion } from "motion/react";
import { Presence } from "../../lib/Motion";
import { motionDurations, motionEases } from "../../tokens/tokens";
import { Card, type CardProps } from "../Card/Card";
import { Switch } from "../Switch/Switch";

export type ToggleSectionProps = {
  title: ReactNode;
  description?: ReactNode;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
  /** Settings shown while it is on. */
  children?: ReactNode;
} & Omit<CardProps, "title" | "description" | "headerTrailing" | "children">;

/** A card switched on and off from its header (X17); its settings show while on. */
export function ToggleSection({ title, description, checked, onCheckedChange, disabled, children, ...rest }: ToggleSectionProps) {
  const id = useId();
  return (
    <Card
      title={<span id={id}>{title}</span>}
      description={description}
      headerTrailing={<Switch checked={checked} onCheckedChange={onCheckedChange} disabled={disabled} aria-labelledby={id} />}
      {...rest}
    >
      {/* The settings unfold with the switch, rather than snapping into place. */}
      <Presence>
        {checked && (
          <motion.div
            key="settings"
            initial={{ height: 0, opacity: 0 }}
            animate={{
              height: "auto",
              opacity: 1,
              transition: { duration: motionDurations.base, ease: motionEases.arrive },
            }}
            exit={{
              height: 0,
              opacity: 0,
              transition: { duration: motionDurations.fast, ease: motionEases.standard },
            }}
            className="overflow-hidden"
          >
            {children}
          </motion.div>
        )}
      </Presence>
    </Card>
  );
}
