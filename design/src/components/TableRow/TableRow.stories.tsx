import type { Meta, StoryObj } from "@storybook/react-vite";
import { IconButton } from "../IconButton/IconButton";
import { Status } from "../Status/Status";
import { TableHeader } from "../TableHeader/TableHeader";
import { StateGrid, statePseudo } from "../../stories/StateGrid";
import { TableRow, TableTitle } from "./TableRow";

const row = (title: string, schedule: string, state: React.ComponentProps<typeof Status>["state"], who: string) => [
  <TableTitle title={title} subtitle={schedule} />,
  <Status state={state} font="sans" />,
  <span className="type-ui text-muted">{who}</span>,
  <IconButton variant="ghost" icon="more" aria-label={`More for ${title}`} />,
];

const meta = {
  title: "Components/Surfaces & data/TableRow",
  component: TableRow,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D38", replaces: null } },
  args: { cells: row("Morning brief", "every weekday · 07:00", "answered", "ganesha"), endAligned: [3], onSelect: () => {} },
  decorators: [
    (Story) => (
      <table className="w-760 border-collapse">
        <colgroup>
          <col className="w-280" />
          <col className="w-200" />
          <col className="w-150" />
          <col />
        </colgroup>
        <tbody>{Story()}</tbody>
      </table>
    ),
  ],
} satisfies Meta<typeof TableRow>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Selected: Story = { args: { selected: true } };

export const States: Story = {
  tags: ["!autodocs"],
  parameters: statePseudo,
  decorators: [(Story) => <div>{Story()}</div>],
  render: () => {
    const t = (props: Partial<React.ComponentProps<typeof TableRow>> = {}) => (
      <table className="w-640 border-collapse">
        <tbody>
          <TableRow cells={row("Morning brief", "every weekday · 07:00", "idle", "ganesha")} endAligned={[3]} onSelect={() => {}} {...props} />
        </tbody>
      </table>
    );
    return (
      <StateGrid
        rows={[
          { cells: [{ label: "Default", node: t() }, { label: "Hover", pseudo: "hover", node: t() }] },
          { cells: [{ label: "Selected", node: t({ selected: true }) }, { label: "Focus", pseudo: "focus-visible", node: t() }] },
        ]}
      />
    );
  },
};

/** With its header, as in the Tasks table. */
export const WithHeader: Story = {
  decorators: [(Story) => <div>{Story()}</div>],
  render: () => (
    <table className="w-760 border-collapse">
      <colgroup>
        <col className="w-280" />
        <col className="w-200" />
        <col className="w-150" />
        <col />
      </colgroup>
      <TableHeader columns={[{ label: "Task" }, { label: "Latest run" }, { label: "Requested by" }, { label: "", srLabel: "Actions" }]} />
      <tbody>
        <TableRow cells={row("Morning brief", "every weekday · 07:00", "answered", "ganesha")} endAligned={[3]} onSelect={() => {}} />
        <TableRow cells={row("Weekly memory tidy", "Sundays · 03:00", "no-response", "Archivist")} endAligned={[3]} onSelect={() => {}} />
        <TableRow cells={row("Price watch", "every hour", "running", "ganesha")} endAligned={[3]} onSelect={() => {}} selected />
        <TableRow cells={row("Quarterly report", "once · 1 Jan", "not-run", "Treasurer")} endAligned={[3]} onSelect={() => {}} />
      </tbody>
    </table>
  ),
};
