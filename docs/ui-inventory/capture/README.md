# Regenerating the screenshots

These scripts start from an empty Vizier instance, seed it with demo data (offline `dummyplug` agents and fake credentials), then capture every page with headless Chromium through Playwright.

```sh
# 1. Build, and start a throwaway instance on port 9911 with a fresh data dir
cargo build
DATA=$(mktemp -d)
VIZIER_JWT_SECRET=demo target/debug/vizier run --data-dir "$DATA" --port 9911 &

# 2. Install the capture tooling (run inside this folder)
cd docs/ui-inventory/capture
npm install
PLAYWRIGHT_BROWSERS_PATH=$PWD/browsers npx playwright install chromium

# 3. Capture, in this order
PLAYWRIGHT_BROWSERS_PATH=$PWD/browsers node onboard.mjs   # needs the instance un-setup
node seed.mjs                                             # admin/demo-password, agents, memories, tasks, skills, users; writes ./token
node chat.mjs                                             # chat history over WebSocket (tool calls, a checkpoint)
# optional: wait about a minute so the one-time "check-inbox" task fires, giving Tasks a run to show
PLAYWRIGHT_BROWSERS_PATH=$PWD/browsers node shots.mjs     # all the other screenshots, into ../screenshots
# re-shoot a subset: node shots.mjs '^(07|15)'
```

`seed.mjs` doesn't create the `check-inbox` one-time task or trigger a dream. For the original capture, both were done by hand with curl right after seeding:

```sh
T=$(cat token)
curl -XPOST localhost:9911/api/v1/agents/aria/tasks -H "authorization: Bearer $T" -H 'content-type: application/json' \
  -d "{\"slug\":\"check-inbox\",\"title\":\"Check inbox once\",\"instruction\":\"tools\",\"schedule\":{\"type\":\"OneTime\",\"datetime\":\"$(date -u -d '+40 seconds' +%FT%TZ)\"}}"
curl -XPOST localhost:9911/api/v1/agents/aria/dream/trigger -H "authorization: Bearer $T"
```

Memory graph nodes are found by scanning the canvas pixels for the node colour, so if the graph styling changes, `nodePoints()` in `shots.mjs` will need updating.
