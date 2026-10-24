import { useId, type HTMLAttributes, type ReactNode } from "react";
import { cx } from "../../lib/cx";
import { Segment, SegmentGroupId } from "../Segment/Segment";

export type FilterTabsProps<V extends string> = {
  options: { value: V; label: ReactNode; count?: ReactNode }[];
  value: V;
  onChange: (value: V) => void;
  "aria-label": string;
} & Omit<HTMLAttributes<HTMLDivElement>, "onChange">;

/**
 * A segmented filter over one list (D17), or a mode switch when the options
 * have no counts (Rich/Markdown/Split, Local/Docker, 7 days/30 days/Custom).
 */
export function FilterTabs<V extends string>({ options, value, onChange, className, ...rest }: FilterTabsProps<V>) {
  const id = useId();
  return (
    <SegmentGroupId.Provider value={id}>
      <div role="group" className={cx("inline-flex rounded-md border border-border-strong bg-surface p-2", className)} {...rest}>
        {options.map((o) => (
          <Segment key={o.value} selected={o.value === value} count={o.count} onClick={() => onChange(o.value)}>
            {o.label}
          </Segment>
        ))}
      </div>
    </SegmentGroupId.Provider>
  );
}
