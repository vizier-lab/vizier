import type { Meta, StoryObj } from "@storybook/react-vite";
import { ScriptCall } from "../ScriptCall/ScriptCall";
import { RunDetail } from "./RunDetail";

const CODE = `hits = memory_search(
    query="vector db benchmark", limit=8)
notes = [memory_read(h["bundle"], h["path"])
         for h in hits[:4]]
rows = [bench_row(n) for n in notes if n]
print(f"parsed {len(rows)} rows")
memory_write(bundle="infra",
    path="vector-dbs/summary.md", body=render_table(rows))
rows`;

const calls = (
  <div className="flex flex-col gap-4">
    <ScriptCall name="memory_search" args='"vector db benchmark"' duration="0.4s" />
    <ScriptCall name="memory_read" repeat={4} args="infra/qdrant.md, +3" duration="0.9s" />
    <ScriptCall
      name="memory_write"
      args="infra/vector-dbs/summary.md"
      duration="0.2s"
      details={<pre className="whitespace-pre-wrap">{'bundle  "infra"\npath    "vector-dbs/summary.md"\nbody    "| Database | p50 | p99 | RAM |…"'}</pre>}
    />
  </div>
);

const meta = {
  title: "Components/Chat/RunDetail",
  component: RunDetail,
  tags: ["approved"],
  parameters: { vizier: { id: "X48", replaces: "webui/app/components/ExecutionReportView.tsx" } },
  args: {
    description: "Compare the benchmark notes for the three databases, render one table, and file it in infra.",
    code: CODE,
    calls,
    callCount: 6,
    output: "parsed 3 rows\nskipped 1 note: no p99 column",
    result: '[{"db": "qdrant", "p50_ms": 4.1, "p99_ms": 19}, {"db": "sqlite-vec", "p50_ms": 6.9, "p99_ms": 31}]',
  },
  decorators: [(Story) => <div className="w-560 rounded-lg bg-surface p-16">{Story()}</div>],
} satisfies Meta<typeof RunDetail>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Ok: Story = {};

/** The script raised: where, and the message; the full traceback on demand. */
export const Raised: Story = {
  args: {
    output: undefined,
    calls: <ScriptCall name="memory_read" repeat={4} args="infra/qdrant.md, +3" duration="0.1s" />,
    callCount: 4,
    traceback: {
      where: 'line 4, in <module>\n    p99 = float(row["p99"])',
      message: "KeyError: 'p99'",
      full: 'Traceback (most recent call last):\n  File "<script>", line 4, in <module>\n    p99 = float(row["p99"])\nKeyError: \'p99\'',
    },
  },
};

/** A sandbox limit stopped it. */
export const Limit: Story = {
  args: {
    output: undefined,
    result: undefined,
    calls: <ScriptCall name="memory_search" repeat={50} args='"p99 shard 1…50"' duration="22.4s" />,
    callCount: 50,
    limit: { title: "Stopped at the 30 s script limit", detail: "Ran 50 calls before it was stopped. Partial results are not returned." },
  },
};
