install:
  @cargo install cargo-watch
  @echo -e "\\e[1;32minstalling core dependencies\\e[0m\n"
  cargo fetch
  @echo -e "\n\\e[1;32mDone\\e[0m"
  @echo -e "\n\\e[1;34minstalling webui dependencies\\e[0m\n"
  git submodule update --init --recursive
  cd design && npm i
  cd webui && npm i
  cd dummyplug && npm ci
  cd landing && VIZIER_SOURCE=.. node scripts/link-design.mjs && VIZIER_SOURCE=.. npm i
  @echo -e "\n\\e[1;32mDone\\e[0m"

# Check out a branch in the workspace and the same-named branch in each submodule, then pull.
# A submodule without that branch is left on the commit the workspace pins.
checkout branch:
  #!/usr/bin/env bash
  set -euo pipefail
  git fetch --all --prune
  git checkout "{{branch}}"
  git pull --ff-only || echo "no upstream to pull for {{branch}}"
  git submodule update --init
  for sm in $(git submodule --quiet foreach 'echo $sm_path'); do
    (
      cd "$sm"
      git fetch --all --prune --quiet
      if git show-ref --verify --quiet "refs/heads/{{branch}}" || git show-ref --verify --quiet "refs/remotes/origin/{{branch}}"; then
        git checkout "{{branch}}"
        git pull --ff-only || echo "$sm: no upstream to pull for {{branch}}"
        echo -e "\e[1;32m$sm -> {{branch}}\e[0m"
      else
        echo -e "\e[1;33m$sm: no branch {{branch}}, left on the pinned commit\e[0m"
      fi
    )
  done

run:
  @cargo run -- run --config dev.vizier.yaml

run-d:
  @cargo run -- run -d --config dev.vizier.yaml

shutdown:
  @cargo run -- shutdown --config dev.vizier.yaml

dev:
  cargo watch -s "just run"

# Jaeger all-in-one for local traces: UI on http://localhost:16686, OTLP on 4317 (gRPC) / 4318 (HTTP).
jaeger:
  @docker rm -f vizier-jaeger >/dev/null 2>&1 || true
  docker run -d --name vizier-jaeger -p 16686:16686 -p 4317:4317 -p 4318:4318 jaegertracing/jaeger:2.22.0
  @echo -e "\n\e[1;32mJaeger UI: http://localhost:16686\e[0m"

jaeger-down:
  @docker rm -f vizier-jaeger

# `just run` exporting traces to the local Jaeger (`just jaeger` first). Jaeger takes no
# metrics, so they're off here; point OTEL_EXPORTER_OTLP_ENDPOINT at a collector to get them.
# Prompts and replies are captured on the spans: fine locally, never in production.
run-otel:
  OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318 OTEL_METRICS_EXPORTER=none OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true cargo run -- run --config dev.vizier.yaml

dev-otel:
  cargo watch -s "just run-otel"

docker:
  @docker-compose down && docker-compose up -d

build:
  @cd webui && npm run build

# The website (landing page, blog and docs) lives in the landing/ submodule and
# is built from the same docs/ and design/ sources as this checkout.
landing:
  @cd landing && VIZIER_SOURCE=.. npm run dev

landing-build:
  @cd landing && VIZIER_SOURCE=.. npm run build

landing-sync:
  @cd landing && VIZIER_SOURCE=.. npm run sync

release:
  @cargo build --release

catalogue-update:
  python3 -I scripts/catalogue_update.py

# Unit tests for the submodule PR linking/cascade automation.
# Branch and dirty state of the workspace and every submodule.
status:
  @git status -sb
  @git submodule foreach --quiet 'printf "\n\033[1;34m%s\033[0m\n" "$sm_path"; git status -sb'

# Pull the current branch of the workspace and every submodule (fast-forward only).
pull:
  git pull --ff-only
  git submodule foreach 'git pull --ff-only || true'

test:
  cargo test

lint:
  cargo clippy

typecheck:
  cd webui && npm run typecheck

# Pre-PR gate: clippy, cargo tests and the WebUI typecheck.
check: lint test typecheck

# Regenerate webui/app/api/schema.d.ts from a running server (default http://localhost:9999, override with VIZIER_URL).
openapi:
  node scripts/openapi.mjs

test-scripts:
  node --test scripts/*.test.mjs

# Optional standalone model server. `just run` never starts this service.
dummyplug:
  npm --prefix dummyplug start

dummyplug-test:
  npm --prefix dummyplug test
