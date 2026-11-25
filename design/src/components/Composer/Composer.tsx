import { useId, useRef, useState, type HTMLAttributes, type KeyboardEvent, type ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { ContextRing } from "../ContextRing/ContextRing";
import { useDismiss } from "../../lib/dismiss";
import { SkillMenu, type SkillMenuItem } from "../SkillMenu/SkillMenu";
import { MentionMenu, type MentionMenuItem } from "../MentionMenu/MentionMenu";
import { Presence } from "../../lib/Motion";
import { Tooltip } from "../Tooltip/Tooltip";

/** A trigger menu the host drives while the person types after "@" (files of a project chat). */
export type ComposerSuggest = {
  /** The list's heading: "Files". */
  label: string;
  /** What was typed after the trigger, for highlighting and the empty state. */
  query: string;
  items: MentionMenuItem[];
  onPick: (item: MentionMenuItem) => void;
  /** Escape: the host puts the menu away until the text changes. */
  onDismiss: () => void;
  /** Said when nothing matches; defaults to the file wording. */
  emptyText?: string;
  /** The menu's footer line; defaults to the mention wording. */
  footer?: string;
};

export type ComposerProps = {
  agentName: string;
  value: string;
  onValueChange: (value: string) => void;
  /**
   * Sends the text (or "/skill input"). While the agent is busy, Ctrl+Enter (Cmd+Enter on macOS)
   * sends with `queue: true`: run it as a new turn after the current one, instead of adding it to
   * the turn in progress.
   */
  onSend: (options?: { queue: boolean }) => void;
  /** The agent is working: Send turns into Stop, which sends /abort. With text typed, Send adds the message to the turn in progress. */
  busy?: boolean;
  /** One muted line under the box, e.g. the busy shortcut: "Enter adds to the current turn · Ctrl+Enter queues it". */
  hint?: ReactNode;
  onStop?: () => void;
  /** Can't send: the reason replaces the placeholder ("Archivist can't answer until it restarts"). */
  disabledReason?: string;
  /** Attachment chips above the text. */
  attachments?: ReactNode;
  onAttach?: () => void;
  onMic?: () => void;
  /** Offer the mic button. Off for an agent that can't transcribe speech. */
  mic?: boolean;
  /** The mic is recording (it turns live). */
  recording?: boolean;
  /** Voice mode: a VoiceRecorder row in place of the text. */
  voice?: ReactNode;
  /** A live voice call's `CallRow`, above the text: typing stays possible during a call. */
  call?: ReactNode;
  /** In the tool row after Attach, "/" and the mic: a host's own toggle, e.g. a spoken reply. */
  afterTools?: ReactNode;
  /** In the right group, just left of Send: a host's own switch, e.g. a chat's Plan/Build mode. */
  beforeSend?: ReactNode;
  /**
   * Starts a live voice call (spec 055). Given, Send shows the voice icon while there is nothing to
   * send, and turns back into Send as soon as there is. Leave it out for an agent with no voice.
   */
  onTalk?: () => void;
  /** Laid over the input box exactly (a DropZone overlay while files are dragged in). */
  overlay?: ReactNode;
  /** The agent's skills, for the slash menu. */
  skills?: SkillMenuItem[];
  /** The "@" menu, when the host has a token to complete. The composer owns its keys, like the slash menu's. */
  suggest?: ComposerSuggest | null;
  /** The context meter, at the end of the tool row's left group. */
  context?: { used: number; window: number; detail?: string };
  /** Mobile shows the count as text instead of the ring: "21k / 200k". */
  tokenText?: string;
  /** Replaces "Message {agentName} — type / for skills", e.g. in a Workspace topic. */
  placeholder?: string;
  /** In a ComposerDeck: a solid box casting its shadow upward onto the back panel standing on it. */
  lifted?: boolean;
} & Omit<HTMLAttributes<HTMLDivElement>, "onChange">;

/**
 * Where a person writes (D46). Attach, "/", voice and the context ring on the
 * left; a host's switch and Send on the right; a host's toggle can end the
 * left group. "/" opens the skill menu. Text entry focuses emerald. Enter
 * sends, Shift+Enter breaks the line; while busy, Ctrl/Cmd+Enter queues.
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
  mic = true,
  recording,
  voice,
  call,
  afterTools,
  beforeSend,
  onTalk,
  overlay,
  skills = [],
  suggest = null,
  placeholder,
  context,
  tokenText,
  hint,
  lifted,
  className,
  ...rest
}: ComposerProps) {
  const id = useId();
  const [active, setActive] = useState(0);
  const disabled = !!disabledReason;
  const slash = /^\/(\S*)$/.exec(value);
  const matches = slash ? skills.filter((s) => s.name.toLowerCase().includes(slash[1].toLowerCase())) : [];
  // Escape or a press elsewhere puts the menu away until the text changes.
  const [put, setPut] = useState<string | null>(null);
  const menuOpen = !!slash && skills.length > 0 && put !== value;
  const menuRoot = useRef<HTMLDivElement>(null);
  useDismiss(menuOpen, () => setPut(value), menuRoot);
  const suggestOpen = !!suggest && !disabled;
  const [suggestActive, setSuggestActive] = useState(0);
  const suggestCount = suggest?.items.length ?? 0;
  const suggestIndex = Math.min(suggestActive, Math.max(0, suggestCount - 1));
  const listId = suggestOpen ? `${id}-mentions` : `${id}-skills`;
  const hasContent = value.trim().length > 0 || !!attachments || !!voice;
  // While the agent works, typing something turns Stop back into Send: the message queues.
  const canSend = !disabled && hasContent;
  const showStop = busy && !hasContent;
  const showTalk = !!onTalk && !busy && !disabled && !hasContent;

  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (suggest && suggestOpen) {
      if (e.key === "Escape") {
        e.preventDefault();
        suggest.onDismiss();
        return;
      }
      if (suggestCount) {
        if (e.key === "ArrowDown" || e.key === "ArrowUp") {
          e.preventDefault();
          setSuggestActive((a) => (Math.min(a, suggestCount - 1) + (e.key === "ArrowDown" ? 1 : suggestCount - 1)) % suggestCount);
          return;
        }
        // Tab and Enter both complete the mention; Enter never sends while the list is open.
        if (e.key === "Tab" || (e.key === "Enter" && !e.shiftKey)) {
          e.preventDefault();
          suggest.onPick(suggest.items[suggestIndex]);
          return;
        }
      }
    }
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
      if (menuOpen && matches.length) {
        onValueChange(`/${matches[active].name}`);
        onSend();
        return;
      }
      // Ctrl+Enter (Cmd+Enter on macOS) queues while the agent works; idle, it simply sends.
      const queue = !!busy && (e.ctrlKey || e.metaKey);
      if (canSend) onSend(queue ? { queue } : undefined);
    }
  };

  const toolButton = "flex size-28 items-center justify-center rounded-sm text-muted hover:bg-raised hover:text-fg disabled:pointer-events-none disabled:opacity-40";

  return (
    <div className={cx("flex flex-col gap-10", className)} {...rest}>
      <div ref={menuRoot} className="relative">
        {/* Presence, so a menu is seen folding back into the composer as it closes. */}
        <Presence>
        {suggestOpen && suggest && (
          <MentionMenu
            key="mentions"
            id={`${id}-mentions`}
            label={suggest.label}
            items={suggest.items}
            query={suggest.query}
            emptyText={suggest.emptyText}
            footer={suggest.footer}
            active={suggestIndex}
            onPick={suggest.onPick}
            // Where the composer deck stands: inset, on the composer's top edge, over its lip.
            className="absolute inset-x-24 bottom-full z-20 max-lg:inset-x-12"
          />
        )}
        {menuOpen && !suggestOpen && (
          <SkillMenu
            key="skills"
            id={`${id}-skills`}
            items={matches}
            query={slash?.[1] ?? ""}
            active={Math.min(active, Math.max(0, matches.length - 1))}
            onPick={(s) => onValueChange(`/${s.name} `)}
            className="absolute inset-x-24 bottom-full z-20 max-lg:inset-x-12"
          />
        )}
        </Presence>
        <div
          className={cx(
            "flex flex-col gap-10 rounded-xl border px-14 pt-14 pb-12",
            lifted ? "bg-surface shadow-lift" : "bg-surface/70 backdrop-blur-md",
            disabled ? "border-border" : "border-border-strong focus-within:border-live",
          )}
        >
          {call}
          {attachments && <div className="flex flex-wrap gap-6">{attachments}</div>}
          {voice ?? (
            <textarea
              rows={1}
              value={value}
              disabled={disabled}
              onChange={(e) => {
                onValueChange(e.target.value);
                setActive(0);
                setSuggestActive(0);
              }}
              onKeyDown={onKeyDown}
              placeholder={disabledReason ?? (recording ? "" : (placeholder ?? `Message ${agentName} — type / for skills`))}
              aria-label={`Message ${agentName}`}
              role={menuOpen || suggestOpen ? "combobox" : undefined}
              aria-expanded={menuOpen || suggestOpen || undefined}
              aria-controls={menuOpen || suggestOpen ? listId : undefined}
              aria-activedescendant={
                suggestOpen ? (suggestCount ? `${listId}-${suggestIndex}` : undefined) : menuOpen && matches.length ? `${listId}-${active}` : undefined
              }
              className="field-sizing-content max-h-240 min-h-24 w-full resize-none bg-transparent type-body text-fg caret-live outline-none placeholder:text-faint disabled:cursor-not-allowed"
            />
          )}
          {recording && !voice && <p className="type-body text-fg">Recording · press the mic again to stop</p>}
          {/* Every tool is a 28px box, 4px apart, so the icons sit on an even 32px rhythm. */}
          <div className="flex items-center gap-4">
            {/* A host that takes no files passes no handler, so there is no button. */}
            {onAttach && (
              <button type="button" aria-label="Attach files" onClick={onAttach} disabled={disabled} className={toolButton}>
                <Icon name="attach" />
              </button>
            )}
            {skills.length > 0 && (
              <button type="button" aria-label="Skills" onClick={() => onValueChange("/")} disabled={disabled} className={toolButton}>
                <Icon name="slash" />
              </button>
            )}
            {mic && (
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
            )}
            {afterTools}
            {tokenText ? (
              <span className="type-mono-xs text-faint">{tokenText}</span>
            ) : (
              context && (
                // A 28px box like the tool buttons, so the ring keeps their 36px rhythm.
                <span className="flex size-28 shrink-0 items-center justify-center">
                  <ContextRing used={context.used} window={context.window} detail={context.detail} />
                </span>
              )
            )}
            <span className="flex-1" />
            {beforeSend}
            {showStop ? (
              <button
                type="button"
                aria-label="Stop"
                onClick={onStop}
                className="flex size-34 items-center justify-center rounded-pill bg-ink text-ground hover:bg-ink-hover active:bg-ink-pressed"
              >
                <span aria-hidden className="size-10 rounded-xs bg-ground" />
              </button>
            ) : showTalk ? (
              <Tooltip label="Live voice">
                <button
                  type="button"
                  aria-label="Start live voice"
                  onClick={onTalk}
                  className="flex size-34 items-center justify-center rounded-pill bg-ink text-ground hover:bg-ink-hover active:bg-ink-pressed"
                >
                  <Icon name="audio-lines" size={16} />
                </button>
              </Tooltip>
            ) : (
              <button
                type="button"
                aria-label={busy ? "Add to the current turn" : "Send"}
                onClick={() => onSend()}
                disabled={!canSend}
                className="flex size-34 items-center justify-center rounded-pill bg-ink text-ground enabled:hover:bg-ink-hover enabled:active:bg-ink-pressed disabled:opacity-40"
              >
                <Icon name="send" size={16} />
              </button>
            )}
          </div>
        </div>
        {overlay}
      </div>
      {hint && <p className="px-4 type-small text-faint">{hint}</p>}
    </div>
  );
}
