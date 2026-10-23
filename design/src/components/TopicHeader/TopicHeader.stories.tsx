import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { ListItem } from "../ListItem/ListItem";
import { Kbd } from "../Kbd/Kbd";
import { Picker } from "../Picker/Picker";
import { TopicHeader } from "./TopicHeader";

const meta = {
  title: "Components/Chat/TopicHeader",
  component: TopicHeader,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X42", replaces: null } },
  args: { topic: "vector-db-review", meta: "webui · topic 3 of 5 · started 11:40" },
} satisfies Meta<typeof TopicHeader>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Click it: the topic Picker opens. */
export const WithPicker: Story = {
  decorators: [(Story) => <div className="h-280">{Story()}</div>],
  render: function Render(args) {
    const [q, setQ] = useState("");
    const topics = [
      { name: "vector-db-review", when: "now" },
      { name: "release-0.13", when: "2 h" },
      { name: "onboarding-copy", when: "yesterday" },
    ].filter((t) => t.name.includes(q));
    return (
      <Picker
        label="Topics"
        trigger={<TopicHeader {...args} />}
        width={396}
        itemHeight={33}
        search={{ placeholder: "Search 214 topics", value: q, onChange: setQ }}
        sections={[{ items: topics }]}
        getKey={(t) => t.name}
        renderItem={(t) => <ListItem shape="bare" className="w-full" title={t.name} titleMono meta={t.when} />}
        onSelect={() => {}}
        footer={
          <span className="flex w-full items-center justify-between">
            <span className="type-code text-fg">+ {q || "new topic"}</span>
            <Kbd>Enter</Kbd>
          </span>
        }
      />
    );
  },
};
