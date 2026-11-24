// Complete, valid interfaces the agent can adapt, returned by `ui_components(recipe=…)`. Each
// is validated against the catalogue in `recipes.test.ts`, so a recipe that stops being valid
// fails the build, not an agent's turn.
import type { UiSpec } from '../validate'
import { boardGame } from './board_game'
import { canvas2d } from './canvas_2d'
import { dashboard } from './dashboard'
import { form } from './form'
import { scene3d } from './scene_3d'
import { tabsAndFilter } from './tabs_and_filter'

export type Recipe = { intent: string; notes: string[]; spec: UiSpec }

export const RECIPES: Record<string, Recipe> = {
    dashboard,
    form,
    tabs_and_filter: tabsAndFilter,
    board_game: boardGame,
    scene_3d: scene3d,
    canvas_2d: canvas2d,
}

export const RECIPE_NAMES = Object.keys(RECIPES)
