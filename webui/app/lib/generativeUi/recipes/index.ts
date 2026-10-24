// Complete, valid interfaces the agent can adapt, returned by `ui_components(recipe=…)`. Each
// is validated against the catalogue in `recipes.test.ts`, so a recipe that stops being valid
// fails the build, not an agent's turn.
import type { UiSpec } from '../validate'
import { dashboard } from './dashboard'
import { form } from './form'
import { tabsAndFilter } from './tabs_and_filter'

export type Recipe = { intent: string; notes: string[]; spec: UiSpec }

export const RECIPES: Record<string, Recipe> = {
    dashboard,
    form,
    tabs_and_filter: tabsAndFilter,
}

export const RECIPE_NAMES = Object.keys(RECIPES)
