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
test-scripts:
  node --test scripts/*.test.mjs

# Optional standalone model server. `just run` never starts this service.
dummyplug:
  npm --prefix dummyplug start

dummyplug-test:
  npm --prefix dummyplug test
