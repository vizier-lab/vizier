// Sample data for the Picker and AgentSwitcher stories. Not exported from the package.
export type AgentItem = { id: string; name: string; state: "live" | "idle" | "failed"; detail: string; owner?: boolean };

const NAMES = ["Almanac", "Auditor", "Bard", "Cartographer", "Chamberlain", "Courier", "Envoy", "Falconer", "Herald", "Jester", "Keeper", "Librarian", "Mason", "Oracle", "Page", "Quartermaster", "Sentinel", "Steward", "Tutor", "Warden"];

export const recentAgents: AgentItem[] = [
  { id: "archivist", name: "Archivist", state: "live", detail: "replying", owner: true },
  { id: "scout", name: "Scout", state: "live", detail: "replying" },
  { id: "scribe", name: "Scribe", state: "failed", detail: "retry in 40s" },
];

/** 1,000 agents, for the scale story. */
export const manyAgents: AgentItem[] = Array.from({ length: 1000 }, (_, i): AgentItem => {
  const base = NAMES[i % NAMES.length];
  return {
    id: `${base.toLowerCase()}-${i}`,
    name: i < NAMES.length ? base : `${base} ${Math.floor(i / NAMES.length) + 1}`,
    state: i % 17 === 0 ? "failed" : i % 5 === 0 ? "live" : "idle",
    detail: i % 17 === 0 ? "retry in 40s" : i % 5 === 0 ? "replying" : `${(i % 23) + 1}h ago`,
    owner: i % 9 === 1,
  };
}).sort((a, b) => a.name.localeCompare(b.name));

export type BundleItem = { name: string; docs: number };

export const bundles: BundleItem[] = [
  "client-projects:64", "project-atlas:212", "project-harbor:17", "projects/2025-archive:1402", "projects/2026-q4:88", "side-projects:9",
  "infra:42", "journal:310", "people:57", "recipes:23", "vector-db-review:31",
].map((s) => {
  const [name, docs] = s.split(":");
  return { name, docs: Number(docs) };
});

export type ConversationItem = { id: string; title: string; channel: "webui" | "discord" | "telegram"; where: string; time: string; live?: boolean; day: "Today" | "Yesterday" | "Older" };

export const conversations: ConversationItem[] = [
  { id: "c1", title: "vector-db-review", channel: "webui", where: "WebUI · topic 3 of 5", time: "14:02", live: true, day: "Today" },
  { id: "c2", title: "#releases", channel: "discord", where: "Discord · Vizier Lab", time: "13:40", day: "Today" },
  { id: "c3", title: "DM · ana", channel: "telegram", where: "Telegram", time: "11:05", day: "Today" },
  { id: "c4", title: "pricing-notes", channel: "webui", where: "WebUI · 2 topics", time: "18:22", day: "Yesterday" },
  { id: "c5", title: "#general", channel: "discord", where: "Discord · Home server", time: "16:10", day: "Yesterday" },
  { id: "c6", title: "family-chat", channel: "telegram", where: "Telegram", time: "Oct 14", day: "Older" },
  { id: "c7", title: "#support", channel: "discord", where: "Discord · Vizier Lab", time: "Oct 12", day: "Older" },
  { id: "c8", title: "migration-plan", channel: "webui", where: "WebUI · topic 1 of 1", time: "Oct 9", day: "Older" },
];
