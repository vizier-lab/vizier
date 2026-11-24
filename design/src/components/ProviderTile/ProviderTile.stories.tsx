import type { Meta, StoryObj } from "@storybook/react-vite";
import { StateGrid, interactionCells, statePseudo } from "../../stories/StateGrid";
import { ProviderTile } from "./ProviderTile";

const meta = {
  title: "Components/Integrations & settings/ProviderTile",
  component: ProviderTile,
  tags: ["approved"],
  parameters: { vizier: { id: "X65", replaces: "webui/app/components/providers/AddProvider.tsx" } },
  args: { provider: "google", name: "Google", sub: "Gemini" },
  decorators: [(Story) => <div className="w-190">{Story()}</div>],
} satisfies Meta<typeof ProviderTile>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Providers › Add a provider › Popular. */
export const Popular: Story = {
  decorators: [(Story) => <div className="grid w-580 grid-cols-3 gap-10">{Story()}</div>],
  render: () => (
    <>
      <ProviderTile provider="google" name="Google" sub="Gemini" />
      <ProviderTile provider="openrouter" name="OpenRouter" sub="300+ models" />
      <ProviderTile provider="github-copilot" name="GitHub Copilot" sub="sign in with GitHub" />
      <ProviderTile provider="mistral" name="Mistral" />
      <ProviderTile provider="deepseek" name="DeepSeek" />
      <ProviderTile provider="xai" name="xAI" sub="Grok" />
      <ProviderTile provider="llama-cpp" name="llama.cpp" sub="local server" />
      <ProviderTile provider="voyageai" name="Voyage AI" sub="embeddings" />
    </>
  ),
};

export const States: Story = {
  tags: ["!autodocs"],
  parameters: statePseudo,
  decorators: [(Story) => <div>{Story()}</div>],
  render: () => <StateGrid rows={[{ cells: interactionCells(() => <div className="w-190"><ProviderTile provider="google" name="Google" sub="Gemini" /></div>) }]} />,
};
