import type { Meta, StoryObj } from "@storybook/react-vite";
import { DiffView } from "./DiffView";

const meta = {
  title: "Components/Rich text/DiffView",
  component: DiffView,
  tags: ["approved"],
  parameters: { vizier: { id: "X52", replaces: "webui/app/components/VersionHistory.tsx" } },
  args: {
    title: "rev 40 → 41",
    lines: [
      { kind: "ctx", text: "## Lately" },
      { kind: "del", text: "• Ask Scout for web checks when convenient." },
      { kind: "add", text: "• Delegate release-note checks to Scout; it is faster and cheaper for web work." },
      { kind: "add", text: "• Keep benchmark numbers in comparison notes." },
    ],
    restoreLabel: "Restore rev 40",
    onRestore: () => {},
  },
  decorators: [(Story) => <div className="w-352">{Story()}</div>],
} satisfies Meta<typeof DiffView>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Core's History panel. */
export const Default: Story = {};

/** A source diff: hunk headers, line numbers and tinted add/remove rows (the project Changes dialog). */
export const Code: Story = {
  args: {
    variant: "code",
    title: "webui/app/routes/chat.tsx",
    restoreLabel: undefined,
    onRestore: undefined,
    lines: [
      { kind: "hunk", text: "@@ -112,6 +112,7 @@ export function ChatRoute()" },
      { kind: "ctx", text: "const { project } = useProject()", oldNo: 112, newNo: 112 },
      { kind: "ctx", text: "const mode = useChatMode(topic)", oldNo: 113, newNo: 113 },
      { kind: "del", text: "const changes = useGitStatus()", oldNo: 114 },
      { kind: "add", text: "const changes = useGitStatus(project)", newNo: 114 },
      { kind: "add", text: "const [detail, setDetail] = useState<string | null>(null)", newNo: 115 },
      { kind: "ctx", text: "const trail = useTrail(topic)", oldNo: 115, newNo: 116 },
    ],
  },
  decorators: [(Story) => <div className="w-760">{Story()}</div>],
};
