// The component catalogue: the single source for validating a spec, drawing it, the `type` enum
// of `ui_render`'s schema and the reference the agent reads through `ui_components`. A component
// is one entry here plus one in `../registry`. Later groups (media, surface, control) are added
// by their own files and listed below.
import { action } from "./action";
import { data } from "./data";
import { input } from "./input";
import { layout } from "./layout";
import { textGroup } from "./text";
import { GROUPS, type ComponentDef, type Group } from "./define";

export { GROUPS, defineComponent } from "./define";
export type { ChildRule, ComponentDef, Group } from "./define";

const entries: ComponentDef[] = [...layout, ...textGroup, ...data, ...input, ...action];

export const catalog: Record<string, ComponentDef> = Object.fromEntries(entries.map((entry) => [entry.name, entry]));

export const COMPONENT_NAMES = Object.keys(catalog);

/** Components of one group, in catalogue order. */
export function inGroup(group: Group): ComponentDef[] {
  return entries.filter((entry) => entry.group === group);
}

/** The action names a binding may use, with the one line the index shows for each. */
export const ACTIONS: { name: string; summary: string }[] = [
  { name: "setState", summary: "setState {statePath, value}: write a value into state" },
  { name: "pushState", summary: "pushState {statePath, value}: append to a list in state" },
  { name: "removeState", summary: "removeState {statePath, index}: remove a list item" },
  { name: "validateForm", summary: "validateForm {statePath?}: check every input's checks" },
  { name: "sendMessage", summary: "sendMessage {text, data?}: send a message as the person (only after their own click)" },
  { name: "run", summary: "run {fn, args?, into?}: call a function from logic; write its result to into" },
];

export const ACTION_NAMES = ACTIONS.map((a) => a.name);

/** The groups that currently have components, in index order. */
export const ACTIVE_GROUPS = GROUPS.filter((group) => entries.some((entry) => entry.group === group));
