// The component renderers, one per catalogue entry. Each group has its own file so groups can
// be built apart; this file lists them for json-render's `Renderer`.
import type { ComponentRegistry } from '@json-render/react'
import { ButtonC } from './action'
import { ChartC, KeyValueC, ProgressC, StatC, TableC } from './data'
import { CheckboxC, NumberInputC, RadioGroupC, SelectC, SliderC, SwitchC, TextInputC } from './input'
import { CardC, DividerC, FoldC, GridC, RowC, StackC, TabsC } from './layout'
import { BadgeC, CalloutC, CodeC, HeadingC, TextC } from './text'

export const registry: ComponentRegistry = {
    Stack: StackC,
    Row: RowC,
    Grid: GridC,
    Card: CardC,
    Tabs: TabsC,
    Fold: FoldC,
    Divider: DividerC,
    Heading: HeadingC,
    Text: TextC,
    Badge: BadgeC,
    Callout: CalloutC,
    Code: CodeC,
    Stat: StatC,
    Table: TableC,
    KeyValue: KeyValueC,
    Progress: ProgressC,
    Chart: ChartC,
    TextInput: TextInputC,
    NumberInput: NumberInputC,
    Slider: SliderC,
    Select: SelectC,
    Checkbox: CheckboxC,
    Switch: SwitchC,
    RadioGroup: RadioGroupC,
    Button: ButtonC,
}
