import { useSyncExternalStore } from "react";

// In AVATAR_COLORS' order, because a face picks its colours by position: the same order keeps each
// person's shape whatever the colours are.
const ROLES = ["--vz-live", "--vz-ink", "--vz-text-soft", "--vz-chart-4", "--vz-chart-5", "--vz-border"];

let current: string[] | null = null;
let observer: MutationObserver | null = null;
const listeners = new Set<() => void>();

/** The six colours when the page has a scheme painted over the stylesheet, otherwise null. */
function read(): string[] | null {
  const html = document.documentElement;
  if (!ROLES.some((role) => html.style.getPropertyValue(role))) return null;
  const style = getComputedStyle(html);
  return ROLES.map((role) => style.getPropertyValue(role).trim());
}

/** Keeps the same array while nothing changed, as useSyncExternalStore requires. */
function refresh(): boolean {
  const next = read();
  if (next?.join() === current?.join()) return false;
  current = next;
  return true;
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  // Avatars come in long lists, so they share one observer rather than having one each.
  if (!observer) {
    observer = new MutationObserver(() => {
      if (refresh()) listeners.forEach((notify) => notify());
    });
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["style", "data-theme"] });
    if (refresh()) listeners.forEach((notify) => notify());
  }
  return () => {
    listeners.delete(listener);
    if (!listeners.size) {
      observer?.disconnect();
      observer = null;
    }
  };
}

function getSnapshot() {
  // Without an observer nothing is keeping `current` up to date.
  if (!observer) refresh();
  return current;
}

/**
 * The colours for generated faces, which need real hex values, not CSS variables. Null when no scheme
 * has painted them, so the default look keeps its fixed face colours exactly.
 */
export function useSchemeColors(): string[] | null {
  return useSyncExternalStore(subscribe, getSnapshot, () => null);
}
