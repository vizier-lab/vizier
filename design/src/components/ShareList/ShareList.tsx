import { useState, type HTMLAttributes } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { Avatar } from "../Avatar/Avatar";
import { Badge } from "../Badge/Badge";
import { Button } from "../Button/Button";

export type SharePerson = { name: string; owner?: boolean; permission?: string };

export type ShareListProps = {
  people: SharePerson[];
  onRemove: (name: string) => void;
  onShare: (username: string) => void;
} & HTMLAttributes<HTMLDivElement>;

/** Who may chat with an agent (X21): people, their permission, and a field to add someone. */
export function ShareList({ people, onRemove, onShare, className, ...rest }: ShareListProps) {
  const [name, setName] = useState("");
  return (
    <div className={cx("flex flex-col gap-12", className)} {...rest}>
      <ul className="flex flex-col gap-10">
        {people.map((p) => (
          <li key={p.name} className="flex items-center gap-10">
            <Avatar name={p.name} size="sm" />
            <span className="min-w-0 flex-1 truncate type-ui text-fg">{p.name}</span>
            {p.owner ? (
              <Badge tone="rank">Owner</Badge>
            ) : (
              <>
                <span className="type-small text-faint">{p.permission ?? "can chat"}</span>
                <button type="button" aria-label={`Stop sharing with ${p.name}`} onClick={() => onRemove(p.name)} className="flex size-18 items-center justify-center rounded-xs text-faint hover:text-fg">
                  <Icon name="close" size={13} />
                </button>
              </>
            )}
          </li>
        ))}
      </ul>
      <form
        className="flex gap-8"
        onSubmit={(e) => {
          e.preventDefault();
          if (name.trim()) onShare(name.trim());
          setName("");
        }}
      >
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Enter username"
          aria-label="Username to share with"
          className="h-34 min-w-0 flex-1 rounded-md border border-border-strong bg-ground px-12 type-ui text-fg outline-none placeholder:text-faint hover:border-border-hover focus:border-live"
        />
        <Button type="submit" disabled={!name.trim()}>Share</Button>
      </form>
    </div>
  );
}
