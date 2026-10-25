import { useCallback, useLayoutEffect, useRef, type ReactNode } from "react";
import { AnimatePresence, MotionConfig, type AnimatePresenceProps } from "motion/react";
import { motionDurations, motionEases } from "../tokens/tokens";

/**
 * Mount once at the app root. `reducedMotion="user"` honours
 * prefers-reduced-motion automatically: transform and layout animations
 * become instant while opacity and colour (state, not movement) still
 * transition — feedback stays legible either way.
 */
export function MotionProvider({ children }: { children: ReactNode }) {
  return <MotionConfig reducedMotion="user">{children}</MotionConfig>;
}

/**
 * AnimatePresence with `initial={false}`: nothing animates on first mount,
 * so a page never choreographs itself in — only later state changes do.
 */
export function Presence(props: AnimatePresenceProps & { children?: ReactNode }) {
  return <AnimatePresence initial={false} {...props} />;
}

/**
 * The native `<dialog>` half of a modal: shown with showModal() (top layer,
 * focus trap, page scroll lock, Esc → cancel) as soon as `open`, and closed
 * only when Presence reports the exit animation done, so the surface is seen
 * leaving instead of vanishing.
 */
export function useNativeDialog(open: boolean) {
  const ref = useRef<HTMLDialogElement>(null);
  useLayoutEffect(() => {
    const d = ref.current;
    if (d && open && !d.open) d.showModal();
  }, [open]);
  const onExitComplete = useCallback(() => {
    const d = ref.current;
    if (d && d.open && !open) d.close();
  }, [open]);
  return { ref, onExitComplete };
}

/*
 * Shared choreography, one per surface class, so every dialog feels like
 * every dialog. Enter on the arrive ease, exit faster on the standard ease.
 * The small y/x offsets mirror space/6, space/8 and space/12 from the scale.
 */

/** A centered dialog panel: settles in, fades down-and-out. */
export const dialogMotion = {
  initial: { opacity: 0, scale: 1.02, y: 8 },
  animate: {
    opacity: 1,
    scale: 1,
    y: 0,
    transition: { duration: motionDurations.slow, ease: motionEases.arrive },
  },
  exit: {
    opacity: 0,
    scale: 0.98,
    y: 4,
    transition: { duration: motionDurations.base, ease: motionEases.standard },
  },
} as const;

/** A right-hand panel: slides with the surface it belongs to. */
export const slideOverMotion = {
  initial: { x: "100%" },
  animate: { x: 0, transition: { duration: motionDurations.slow, ease: motionEases.arrive } },
  exit: { x: "100%", transition: { duration: motionDurations.base, ease: motionEases.standard } },
} as const;

/** A left-hand panel, for a control anchored on the left (the phone menu). */
export const slideOverLeftMotion = {
  initial: { x: "-100%" },
  animate: { x: 0, transition: { duration: motionDurations.slow, ease: motionEases.arrive } },
  exit: { x: "-100%", transition: { duration: motionDurations.base, ease: motionEases.standard } },
} as const;

/** A menu or picker: fades with a small rise; dismissed even faster. */
export const popoverMotion = {
  initial: { opacity: 0, y: 6 },
  animate: {
    opacity: 1,
    y: 0,
    transition: { duration: motionDurations.base, ease: motionEases.arrive },
  },
  exit: {
    opacity: 0,
    y: 4,
    transition: { duration: motionDurations.fast, ease: motionEases.standard },
  },
} as const;

/** The scrim behind a modal surface. */
export const scrimMotion = {
  initial: { opacity: 0 },
  animate: { opacity: 1, transition: { duration: motionDurations.slow, ease: motionEases.standard } },
  exit: { opacity: 0, transition: { duration: motionDurations.base, ease: motionEases.standard } },
} as const;
