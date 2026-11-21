import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { AudioPlayer } from "./AudioPlayer";

const meta = {
  title: "Components/Chat/AudioPlayer",
  component: AudioPlayer,
  tags: ["approved"],
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

/** A live voice message just heard: the recording plays at once, the transcript is on its way. */
export const Transcribing: Story = { args: { transcribing: true, transcript: undefined, position: 0, duration: 3 } };

/** A live voice reply still being spoken: the recording is not here yet, the transcript already is. */
export const Pending: Story = { args: { pending: true, transcript: "Two meetings tomorrow: stand-up at 9:30 and the vendor call at 11.", position: 0, duration: 0 } };
