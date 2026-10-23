import type { Meta, StoryObj } from "@storybook/react-vite";
import { ProviderLogo } from "./ProviderLogo";

const PROVIDERS: [string, string][] = [
  ["anthropic", "Anthropic"],
  ["openai", "OpenAI"],
  ["chatgpt", "ChatGPT account"],
  ["google", "Google"],
  ["openrouter", "OpenRouter"],
  ["github-copilot", "GitHub Copilot"],
  ["mistral", "Mistral"],
  ["deepseek", "DeepSeek"],
  ["xai", "xAI"],
  ["ollama", "Ollama"],
  ["home-vllm", "Home vLLM"],
];

const meta = {
  title: "Components/Integrations & settings/ProviderLogo",
  component: ProviderLogo,
  tags: ["approved"],
  parameters: {
    vizier: { id: "X63", replaces: "webui/app/components/ProviderLogo.tsx" },
    docs: { description: { component: "Known providers show their letters on their brand colour (`brands.ts`, the one place brand colours live). Local and custom providers get the neutral tile with their initials." } },
  },
  args: { provider: "anthropic", name: "Anthropic", size: "md" },
  argTypes: { size: { control: "inline-radio", options: ["sm", "md"] } },
} satisfies Meta<typeof ProviderLogo>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const All: Story = {
  render: () => (
    <div className="flex flex-col gap-16">
      {(["md", "sm"] as const).map((size) => (
        <div key={size} className="flex flex-wrap items-center gap-12">
          {PROVIDERS.map(([id, name]) => (
            <ProviderLogo key={id} provider={id} name={name} size={size} />
          ))}
        </div>
      ))}
    </div>
  ),
};
