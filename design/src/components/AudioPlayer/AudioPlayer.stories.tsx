import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { AudioPlayer } from "./AudioPlayer";

const meta = {
  title: "Components/Chat/AudioPlayer",
  component: AudioPlayer,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X45", replaces: "webui/app/components/VoiceMessagePlayer.tsx" } },
  args: {
    playing: false,
    onPlayingChange: () => {},
    position: 15,
    duration: 42,
    transcript: "All three handled the 2.1M-passage corpus. sqlite-vec keeps p99 under forty milliseconds on one node, which is why I would start there.",
  },
  decorators: [(Story) => <div className="w-396">{Story()}</div>],
} satisfies Meta<typeof AudioPlayer>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Paused: Story = {
  render: function Render(args) {
    const [playing, setPlaying] = useState(false);
    const [speed, setSpeed] = useState(1);
    return <AudioPlayer {...args} playing={playing} onPlayingChange={setPlaying} speed={speed} onSpeedChange={setSpeed} />;
  },
};

export const Playing: Story = { args: { playing: true, position: 28 } };
