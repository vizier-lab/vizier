import type { Meta, StoryObj } from "@storybook/react-vite";
import { Eyebrow } from "../Eyebrow/Eyebrow";
import { ProviderRow } from "./ProviderRow";

const meta = {
  title: "Components/Integrations & settings/ProviderRow",
  component: ProviderRow,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X64", replaces: "webui/app/components/providers/ConnectedList.tsx" } },
  args: { provider: "anthropic", name: "Anthropic", credential: "Saved key ••••7c1e", usage: "3 agents · 41 models", onModels: () => {}, onMore: () => {} },
  decorators: [(Story) => <div className="w-860 divide-y divide-border rounded-xl border border-border bg-surface">{Story()}</div>],
} satisfies Meta<typeof ProviderRow>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Providers › Connected. */
export const Connected: Story = {
  render: () => (
    <>
      <Eyebrow count={5} className="bg-sidebar px-16 py-12">Connected</Eyebrow>
      <ProviderRow provider="anthropic" name="Anthropic" credential="Saved key ••••7c1e" usage="3 agents · 41 models" onModels={() => {}} onMore={() => {}} />
      <ProviderRow provider="openai" name="OpenAI" credential="Saved key ••••Q2aa" usage="1 agent · 58 models" onModels={() => {}} onMore={() => {}} />
      <ProviderRow provider="chatgpt" name="ChatGPT account" credential="Signed in as ganesha@… · OAuth" usage="not used yet" onModels={() => {}} onMore={() => {}} />
      <ProviderRow provider="ollama" name="Ollama" credential="http://localhost:11434 · no key" usage="2 agents · 12 pulled models" onModels={() => {}} onMore={() => {}} />
      <ProviderRow provider="home-vllm" name="Home vLLM" custom credential="Custom · OpenAI-compatible · http://10.0.0.8:8000/v1" usage="1 model" onModels={() => {}} onMore={() => {}} />
    </>
  ),
};
