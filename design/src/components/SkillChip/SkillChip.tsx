import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";

export type SkillChipProps = { skill: string } & HTMLAttributes<HTMLSpanElement>;

/** The skill a person invoked, on their message (D56). */
export function SkillChip({ skill, className, ...rest }: SkillChipProps) {
  return (
    <span className={cx("inline-flex h-19 items-center rounded-sm bg-ground px-6 type-mono-sm-medium text-fg", className)} {...rest}>
      /{skill.replace(/^\//, "")}
    </span>
  );
}
