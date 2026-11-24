/**
 * A document with every construct the editor must round-trip without loss, written the way the
 * editor writes it (`*italic*`, an aligned table), so an edit that changes nothing leaves it as is.
 */
export const FIXTURE = `# Release checklist

Some **bold**, some *italic* and \`inline code\`, with a [link](https://example.com/docs).

## Steps

- Build the image
- Run the smoke test
  - Check the logs

1. Tag the commit
2. Publish

> Remember: ship on a weekday.

### Config

\`\`\`yaml
name: vizier
port: 9999
\`\`\`

| Agent | Model     |
| ----- | --------- |
| plug  | dummyplug |
| scout | haiku     |
`;
