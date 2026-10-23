import type { Meta, StoryObj } from "@storybook/react-vite";
import { Prose } from "./Prose";

const sample = (
  <>
    <h1>Archivist</h1>
    <p>
      I keep the household's knowledge in order: research notes, decisions and the reasons behind them. I answer to{" "}
      <strong>ganesha</strong> and to anyone they invite.
    </p>
    <h2>How I work</h2>
    <ul>
      <li>File findings in memory with their sources; update a note before creating a new one.</li>
      <li>When I am unsure whether something matters, ask once, then remember the answer.</li>
    </ul>
    <h3>Vector stores compared</h3>
    <table>
      <thead>
        <tr>
          <th>Store</th>
          <th>Index</th>
          <th>Notes</th>
        </tr>
      </thead>
      <tbody>
        <tr>
          <td>sqlite-vec</td>
          <td>flat</td>
          <td>embedded, no service</td>
        </tr>
        <tr>
          <td>Qdrant</td>
          <td>HNSW</td>
          <td>separate server</td>
        </tr>
      </tbody>
    </table>
    <blockquote>Keep benchmark numbers in comparison notes.</blockquote>
    <p>
      Search with <code>memory_search</code>, then read the document:
    </p>
    <pre>
      <code>{'hits = memory_search(query="vector db")\nmemory_read(hits[0]["bundle"], hits[0]["path"])'}</code>
    </pre>
    <ol>
      <li>Find</li>
      <li>Read</li>
      <li>Revise</li>
    </ol>
  </>
);

const meta = {
  title: "Components/Rich text/Prose",
  component: Prose,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X49", replaces: "webui/app/components/MarkdownEditor.tsx" } },
  args: { children: sample },
  decorators: [(Story) => <div className="max-w-640">{Story()}</div>],
} satisfies Meta<typeof Prose>;

export default meta;
type Story = StoryObj<typeof meta>;

/** In a chat reply. */
export const Reply: Story = {};

/** In Core and the skill editor. */
export const Document: Story = { args: { size: "document" } };
