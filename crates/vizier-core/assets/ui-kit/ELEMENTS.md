# Kit elements

The details of each `<vz-*>` element, returned by `ui_describe_elements`. The rules every element follows are in the `ui_render` guide.

### `<vz-card>`

A bordered panel with an optional title, holding any HTML.

| Attribute | Type | |
|---|---|---|
| `title` | string | heading of the card |
| `subtitle` | string | one line under the title |
| `tone` | `default` · `error` | error draws it as a danger card |

Children: any HTML, drawn inside the card (add more later through `card.body`).

```html
<vz-card title="This week" subtitle="Signups by day"><p>Steady growth.</p></vz-card>
```

### `<vz-stat>`

One figure with its label, and an optional change under it.

| Attribute | Type | |
|---|---|---|
| `label` * | string | what the figure is |
| `value` * | number | the figure |
| `format` | `number` · `percent` · `currency` · `bytes` · `duration` | how a number is shown: percent takes a fraction (0.42 is 42%), duration takes seconds |
| `delta` | string | a change to show under it, as text ("+12% on last week") |
| `tone` | `default` · `error` | error draws the figure in the failed colour |

```html
<vz-stat label="Activated" value="0.61" format="percent" delta="+4 pts"></vz-stat>
```

### `<vz-table>`

Rows of data under labelled columns, numbers right-aligned on request.

| Attribute | Type | |
|---|---|---|
| `columns` * | json | list of {key, label, align?: "start"|"end", format?} |
| `rows` | json | list of objects keyed by the column keys |
| `label` | string | what the table holds, for screen readers |
| `max-rows` | number | rows shown before "Showing N of M" (default 200) |
| `selectable` | boolean | rows can be pressed; fires select |

- Event `select`: `detail` is {row, index}: the row pressed (selectable only).

```html
<vz-table label="Scores" columns='[{"key":"name","label":"Name"},{"key":"score","label":"Score","align":"end"}]' rows='[{"name":"Ada","score":91},{"name":"Lin","score":84}]'></vz-table>
```

### `<vz-chart>`

A bar, line, area or pie chart in the theme colours.

| Attribute | Type | |
|---|---|---|
| `kind` * | `bar` · `line` · `area` · `pie` | the kind of chart |
| `data` * | json | list of {label, value, series?}; points with the same series form one line or bar group |
| `title` | string | heading over the chart |
| `height` | number | height in px (default 220) |
| `legend` | boolean | show the series legend |

- Event `press`: `detail` is {label, value}: the bar, point or slice pressed.

```html
<vz-chart kind="bar" title="Signups" data='[{"label":"Mon","value":120},{"label":"Tue","value":180}]'></vz-chart>
```

### `<vz-tabs>`

Tabs over sections: each child `<section label="…">` is one tab.

| Attribute | Type | |
|---|---|---|
| `value` | string | the label of the tab shown first (default the first) |
| `label` | string | what the tabs choose between, for screen readers |

Children: `<section label="…">` elements, one per tab.

- Event `change`: `detail` is {value}: the label of the tab chosen.

```html
<vz-tabs><section label="Summary"><p>…</p></section><section label="Details"><p>…</p></section></vz-tabs>
```

### `<vz-field>`

A labelled input: text, multi-line text, number, select, checkbox, switch or slider.

| Attribute | Type | |
|---|---|---|
| `kind` | `text` · `textarea` · `number` · `select` · `checkbox` · `switch` · `slider` | the kind of input (default text) |
| `label` * | string | the label shown with it |
| `name` | string | a name for your own code; it is in every event |
| `value` | string | the starting value ("true" for a checked checkbox or switch) |
| `placeholder` | string | hint inside an empty text or number field |
| `options` | json | select only: list of strings, or of {value, label} |
| `min` | number | number and slider: the least value |
| `max` | number | number and slider: the greatest value |
| `step` | number | number and slider: the step |
| `help` | string | a line of help under it |
| `disabled` | boolean | cannot be changed |

- Event `input`: `detail` is {name, value}: on every change while typing or dragging.
- Event `change`: `detail` is {name, value}: when a value is committed (and on every change for select, checkbox and switch).

```html
<vz-field kind="select" name="range" label="Range" value="7d" options='["24h","7d","30d"]'></vz-field>
```

### `<vz-badge>`

A small label, its text the element's own text.

| Attribute | Type | |
|---|---|---|
| `tone` | `default` · `accent` · `ok` · `warn` · `error` | colour of the element |

Children: the text of the badge.

```html
<vz-badge tone="ok">live</vz-badge>
```

### `<vz-callout>`

A note set apart from the page: a quiet hint, or a warning or error.

| Attribute | Type | |
|---|---|---|
| `tone` | `default` · `accent` · `ok` · `warn` · `error` | colour of the element |
| `title` | string | heading of the note |

Children: the text of the note (any HTML).

```html
<vz-callout tone="warn" title="Estimates">These numbers are from a sample.</vz-callout>
```

### `<vz-progress>`

A progress bar with its label.

| Attribute | Type | |
|---|---|---|
| `value` * | number | progress from 0 to max |
| `max` | number | the value that is complete (default 1) |
| `label` | string | what is progressing |
| `tone` | `default` · `error` | error draws it as failed |

```html
<vz-progress label="Import" value="0.4"></vz-progress>
```

### `<vz-image>`

An image from the conversation's files or an https address.

| Attribute | Type | |
|---|---|---|
| `file` | string | the name of a file in this conversation |
| `src` | string | an https: or data: address (when there is no file) |
| `alt` * | string | what the image shows |
| `width` | number | width in px |
| `height` | number | height in px |

- Event `load`: `detail` is {}: the image loaded.
- Event `error`: `detail` is {message}: it could not be loaded.

```html
<vz-image file="forecast.png" alt="Forecast for the week"></vz-image>
```
