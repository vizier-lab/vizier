// The state of one drawn interface (data-model §3): a json-render store seeded from the spec's
// `state` and the latest saved state, saved back (minus `_` keys) at most once per 500 ms, and
// merged with another tab's state without counting as a local change.
import { createStateStore, type StateStore } from "@json-render/core";

export const SAVE_INTERVAL_MS = 500;

type Snapshot = Record<string, unknown>;

/** Top-level keys starting with `_` are derived: recomputed on every draw, never saved or synced. */
export function saveable(state: Snapshot): Snapshot {
  return Object.fromEntries(Object.entries(state).filter(([key]) => !key.startsWith("_")));
}

export type PageState = {
  store: StateStore;
  /** Called after a change made here (not a merge from another tab). */
  subscribeLocal: (listener: () => void) => () => void;
  /** State saved by another tab: merged in, not saved back, not a local change. */
  mergeRemote: (state: Snapshot) => void;
  /** Save now if something changed. */
  flush: () => void;
  dispose: () => void;
};

export function createPageState(options: {
  initial?: Snapshot;
  saved?: Snapshot | null;
  save: (state: Snapshot) => void;
  intervalMs?: number;
}): PageState {
  const { initial = {}, saved, save, intervalMs = SAVE_INTERVAL_MS } = options;
  // Saved state wins over the spec's, key by key; derived keys only ever come from the spec.
  const store = createStateStore({ ...initial, ...(saved ? saveable(saved) : {}) });

  let lastSaved = JSON.stringify(saveable(store.getSnapshot()));
  let timer: ReturnType<typeof setTimeout> | null = null;
  let remote = 0;
  const local = new Set<() => void>();

  const writeIfChanged = () => {
    timer = null;
    const state = saveable(store.getSnapshot());
    const json = JSON.stringify(state);
    if (json === lastSaved) return;
    lastSaved = json;
    save(state);
  };

  const unsubscribe = store.subscribe(() => {
    if (remote > 0) return;
    for (const listener of [...local]) listener();
    if (timer === null) timer = setTimeout(writeIfChanged, intervalMs);
  });

  return {
    store,
    subscribeLocal(listener) {
      local.add(listener);
      return () => local.delete(listener);
    },
    mergeRemote(state) {
      const incoming = saveable(state);
      remote++;
      try {
        store.update(Object.fromEntries(Object.entries(incoming).map(([key, value]) => [`/${key}`, value])));
      } finally {
        remote--;
      }
      // What another tab saved is already saved: don't echo it back.
      lastSaved = JSON.stringify(saveable(store.getSnapshot()));
    },
    flush() {
      if (timer !== null) clearTimeout(timer);
      writeIfChanged();
    },
    dispose() {
      unsubscribe();
      if (timer !== null) clearTimeout(timer);
      timer = null;
      local.clear();
    },
  };
}
