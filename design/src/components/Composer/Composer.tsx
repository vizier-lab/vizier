import { useId, useState, type HTMLAttributes, type KeyboardEvent, type ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { ContextRing } from "../ContextRing/ContextRing";
import { SkillMenu, type SkillMenuItem } from "../SkillMenu/SkillMenu";

export type ComposerProps = {
  agentName: string;
  value: string;
  onValueChange: (value: string) => void;
  /** Sends the text (or "/skill input"). */
  onSend: () => void;
  /** The agent is working: Send turns into Stop, which sends /abort. */
  busy?: boolean;
  onStop?: () => void;
  /** Can't send: the reason replaces the placeholder ("Archivist can't answer until it restarts"). */
  disabledReason?: string;
  /** Attachment chips above the text. */
  attachments?: ReactNode;
  onAttach?: () => void;
  onMic?: () => void;
  /** The mic is recording (it turns live). */
  recording?: boolean;
  /** Voice mode: a VoiceRecorder row in place of the text. */
  voice?: ReactNode;
  /** Under the composer: the "Ask for a spoken reply" switch. */
  below?: ReactNode;
  /** The agent's skills, for the slash menu. */
  skills?: SkillMenuItem[];
  /** The context meter beside Send. */
  context?: { used: number; window: number; detail?: string };
  /** Mobile shows the count as text instead of the ring: "21k / 200k". */
  tokenText?: string;
} & Omit<HTMLAttributes<HTMLDivElement>, "onChange">;

/**
 * Where a person writes (D46). Attach and voice on the left, the context
 * ring and Send on the right; "/" opens the skill menu. Text entry focuses
 * emerald. Enter sends, Shift+Enter breaks the line.
 */
export function Composer({
  agentName,
  value,
  onValueChange,
  onSend,
  busy,
  onStop,
  disabledReason,
  attachments,
  onAttach,
  onMic,
  recording,
  voice,
  below,
  skills = [],
  context,
  tokenText,
  className,
  ...rest
}: ComposerProps) {
  const id = useId();
  const [active, setActive] = useState(0);
  const disabled = !!disabledReason;
  const slash = /^\/(\S*)$/.exec(value);
  const matches = slash ? skills.filter((s) => s.name.toLowerCase().includes(slash[1].toLowerCase())) : [];
  const menuOpen = !!slash && skills.length > 0;
  const canSend = !disabled && !busy && (value.trim().length > 0 || !!attachments || !!voice);

  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (menuOpen && matches.length) {
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        setActive((a) => (a + (e.key === "ArrowDown" ? 1 : matches.length - 1)) % matches.length);
        return;
      }
      if (e.key === "Tab") {
        e.preventDefault();
        onValueChange(`/${matches[active].name} `);
        return;
      }
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if (menuOpen && matches.length) onValueChange(`/${matches[active].name}`);
      if (canSend || (menuOpen && matches.length)) onSend();
    }
  };

  const toolButton = "flex size-28 items-center justify-center rounded-sm text-muted hover:bg-raised hover:text-fg disabled:pointer-events-none disabled:opacity-40";

  return (
    <div className={cx("flex flex-col gap-10", className)} {...rest}>
      <div className="relative">
        {menuOpen && (
          <SkillMenu
            id={`${id}-skills`}
            items={matches}
            query={slash?.[1] ?? ""}
            active={Math.min(active, Math.max(0, matches.length - 1))}
            onPick={(s) => onValueChange(`/${s.name} `)}
            className="absolute inset-x-0 bottom-full mb-8"
          />
        )}
        <div
          className={cx(
            "flex flex-col gap-10 rounded-xl border px-14 pt-14 pb-12",
            disabled ? "border-border bg-surface" : "border-border-strong bg-surface focus-within:border-live",
          )}
        >
          {attachments && <div className="flex flex-wrap gap-6">{attachments}</div>}
          {voice ?? (
            <textarea
              rows={1}
              value={value}
              disabled={disabled}
              onChange={(e) => {
                onValueChange(e.target.value);
                setActive(0);
              }}
              onKeyDown={onKeyDown}
              placeholder={disabledReason ?? (recording ? "" : `Message ${agentName} — type / for skills`)}
              aria-label={`Message ${agentName}`}
              role={menuOpen ? "combobox" : undefined}
              aria-expanded={menuOpen || undefined}
              aria-controls={menuOpen ? `${id}-skills` : undefined}
              aria-activedescendant={menuOpen && matches.length ? `${id}-skills-${active}` : undefined}
              className="field-sizing-content max-h-240 min-h-24 w-full resize-none bg-transparent type-body text-fg caret-live outline-none placeholder:text-faint disabled:cursor-not-allowed"
            />
          )}
          {recording && !voice && <p className="type-body text-fg">Recording · press the mic again to stop</p>}
          <div className="flex items-center gap-8">
            <button type="button" aria-label="Attach files" onClick={onAttach} disabled={disabled} className={toolButton}>
              <Icon name="attach" />
            </button>
            <button
              type="button"
              aria-label={recording ? "Stop recording" : "Record a voice message"}
              aria-pressed={recording ?? false}
              onClick={onMic}
              disabled={disabled}
              className={cx(toolButton, recording && "text-live hover:text-live")}
            >
              <Icon name="mic" />
            </button>
            {skills.length > 0 && (
              <button type="button" aria-label="Skills" onClick={() => onValueChange("/")} disabled={disabled} className={toolButton}>
                <Icon name="slash" />
              </button>
            )}
            <span className="flex-1" />
            {tokenText ? (
              <span className="type-mono-xs text-faint">{tokenText}</span>
            ) : (
              context && <ContextRing used={context.used} window={context.window} detail={context.detail} />
            )}
            {busy ? (
              <button
                type="button"
                aria-label="Stop"
                onClick={onStop}
                className="flex size-34 items-center justify-center rounded-pill bg-ink text-ground hover:bg-ink-hover active:bg-ink-pressed"
              >
                <span aria-hidden className="size-10 rounded-xs bg-ground" />
              </button>
            ) : (
              <button
                type="button"
                aria-label="Send"
                onClick={onSend}
                disabled={!canSend}
                className="flex size-34 items-center justify-center rounded-pill bg-ink text-ground enabled:hover:bg-ink-hover enabled:active:bg-ink-pressed disabled:opacity-40"
              >
                <Icon name="send" size={16} />
              </button>
            )}
          </div>
        </div>
      </div>
      {below}
    </div>
  );
}
