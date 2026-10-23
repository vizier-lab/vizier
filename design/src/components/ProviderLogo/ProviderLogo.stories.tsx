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
    docs: { description: { component: "Known providers show their letters on their brand colour (`brands.ts`, the one place brand colours live). With a catalogue logo (`src`) the real mark is drawn on a neutral tile: single-colour marks as a mask in the text colour, so they follow the theme; `fullColor` ones as they are. Local and custom providers without a logo get the neutral tile with their initials." } },
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

const MARK = "data:image/svg+xml;utf8," + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="currentColor" d="M12 2 2 7l10 5 10-5-10-5Zm0 12L2 9v8l10 5 10-5V9l-10 5Z"/></svg>');

export const WithCatalogueLogo: Story = {
  render: () => (
    <div className="flex items-center gap-12">
      <ProviderLogo provider="acme" name="Acme" src={MARK} />
      <ProviderLogo provider="acme" name="Acme" src={MARK} size="sm" />
    </div>
  ),
};
