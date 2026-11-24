import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { ShareList, type SharePerson } from "./ShareList";

const meta = {
  title: "Components/Actions & inputs/ShareList",
  component: ShareList,
  tags: ["approved"],
  parameters: { vizier: { id: "X21", replaces: null } },
  args: {
    people: [{ name: "ganesha", owner: true }, { name: "ana" }, { name: "rio" }],
    onRemove: () => {},
    onShare: () => {},
  },
  decorators: [(Story) => <div className="w-300">{Story()}</div>],
} satisfies Meta<typeof ShareList>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Config's Sharing card. */
export const Default: Story = {
  render: function Render(args) {
    const [people, setPeople] = useState<SharePerson[]>(args.people);
    return (
      <ShareList
        people={people}
        onRemove={(n) => setPeople(people.filter((p) => p.name !== n))}
        onShare={(n) => setPeople([...people, { name: n }])}
      />
    );
  },
};
