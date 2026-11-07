import { useId } from "react";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons/map";
import { Dialog } from "../Dialog/Dialog";

export type CreateChooserProps = {
  open: boolean;
  onWorkspace: () => void;
  onAgent: () => void;
  onClose: () => void;
};

function Choice({ icon, title, hint, onSelect }: { icon: IconName; title: string; hint: string; onSelect: () => void }) {
  const id = useId();
  return (
    <button
      type="button"
      aria-describedby={id}
      onClick={onSelect}
      className="flex items-center gap-14 rounded-lg border border-border-strong px-16 py-14 text-left transition-colors hover:border-border-hover hover:bg-surface"
    >
      <span className="grid size-36 shrink-0 place-items-center rounded-md bg-raised text-soft">
        <Icon name={icon} size={18} />
      </span>
      <span className="flex min-w-0 flex-col gap-2">
        <span className="type-body-strong text-fg">{title}</span>
        <span id={id} className="type-small text-muted">
          {hint}
        </span>
      </span>
    </button>
  );
}

/**
 * The one "+" at the end of the rail asks what to make (X63): a Workspace or an agent. The host skips
 * it when the person may create only one, and hides the "+" when neither. Closes on ×, Escape and a
 * press outside, like every dialog.
 */
export function CreateChooser({ open, onWorkspace, onAgent, onClose }: CreateChooserProps) {
  return (
    <Dialog open={open} title="Create new" confirmLabel="" onConfirm={onClose} onCancel={onClose}>
      <div className="flex flex-col gap-10">
        <Choice icon="users" title="Workspace" hint="A shared space for people and agents, with topics." onSelect={onWorkspace} />
        <Choice icon="agent" title="Agent" hint="One agent with its own tools, memory and model." onSelect={onAgent} />
      </div>
    </Dialog>
  );
}
