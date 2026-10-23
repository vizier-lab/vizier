import { useId, type ReactNode } from "react";
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
      {checked && children}
    </Card>
  );
}
