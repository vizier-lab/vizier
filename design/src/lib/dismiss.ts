import { useEffect, useRef, type PointerEvent, type RefObject } from "react";

/**
 * Props for a modal's backdrop: a click on the backdrop itself (not on the panel over it) calls
 * `onDismiss`. The press has to start there too, so selecting text in a field and letting go
 * outside the panel doesn't close it.
 */
export function useBackdropDismiss(onDismiss: () => void) {
  const pressed = useRef(false);
  return {
    onPointerDown: (e: PointerEvent<HTMLElement>) => {
      pressed.current = e.target === e.currentTarget;
    },
    onClick: (e: { target: EventTarget; currentTarget: EventTarget }) => {
      const hit = pressed.current && e.target === e.currentTarget;
      pressed.current = false;
      if (hit) onDismiss();
    },
  };
}

/**
 * A popup that is not a modal closes on Escape and on a press outside `ref` (and outside
 * `ignore`, usually the control that opens it, which toggles it itself).
 */
export function useDismiss(open: boolean, onDismiss: () => void, ref: RefObject<HTMLElement | null>, ignore?: RefObject<HTMLElement | null>) {
  const dismiss = useRef(onDismiss);
  dismiss.current = onDismiss;
  useEffect(() => {
    if (!open) return;
    const down = (e: globalThis.PointerEvent) => {
      const t = e.target as Node;
      if (ref.current?.contains(t) || ignore?.current?.contains(t)) return;
      dismiss.current();
    };
    const key = (e: globalThis.KeyboardEvent) => e.key === "Escape" && dismiss.current();
    document.addEventListener("pointerdown", down);
    document.addEventListener("keydown", key);
    return () => {
      document.removeEventListener("pointerdown", down);
      document.removeEventListener("keydown", key);
    };
  }, [open, ref, ignore]);
}
