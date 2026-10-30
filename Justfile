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
# A branch that doesn't exist yet is created, from the current commit, in the workspace and in every submodule.
checkout branch:
  #!/usr/bin/env bash
  set -euo pipefail
  has_branch() {
    git show-ref --verify --quiet "refs/heads/{{branch}}" || git show-ref --verify --quiet "refs/remotes/origin/{{branch}}"
  }
  git fetch --all --prune
  if has_branch; then
    git checkout "{{branch}}"
    git pull --ff-only || echo "no upstream to pull for {{branch}}"
    git submodule update --init
  else
    git checkout -b "{{branch}}"
    echo -e "\e[1;32mcreated {{branch}}\e[0m"
  fi
  for sm in $(git submodule --quiet foreach 'echo $sm_path'); do
    (
      cd "$sm"
      git fetch --all --prune --quiet
      if has_branch; then
        git checkout "{{branch}}"
        git pull --ff-only || echo "$sm: no upstream to pull for {{branch}}"
        echo -e "\e[1;32m$sm -> {{branch}}\e[0m"
      else
        git checkout -b "{{branch}}"
        echo -e "\e[1;32m$sm -> {{branch}} (created)\e[0m"
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
  @cargo build --release -p vizier

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

# Commit everything (git add -A) with <message> in each submodule that has changes, then in the workspace, which also records the new submodule pins.
# Repos with nothing to commit are skipped; a submodule on a detached HEAD aborts the run before anything is committed.
commit message:
  #!/usr/bin/env bash
  set -euo pipefail
  msg={{quote(message)}}
  mods=$(git submodule --quiet foreach 'echo $sm_path')
  for sm in $mods; do
    if [ -n "$(git -C "$sm" status --porcelain)" ] && ! git -C "$sm" symbolic-ref -q HEAD >/dev/null; then
      echo -e "\e[1;31m$sm has changes on a detached HEAD, check out a branch first\e[0m"
      exit 1
    fi
  done
  for sm in $mods; do
    if [ -n "$(git -C "$sm" status --porcelain)" ]; then
      git -C "$sm" add -A
      git -C "$sm" commit -q -m "$msg"
      echo -e "\e[1;32m$sm committed\e[0m"
    fi
  done
  git add -A
  if git diff --cached --quiet; then
    echo "workspace: nothing to commit"
  else
    git commit -q -m "$msg"
    echo -e "\e[1;32mworkspace committed\e[0m"
  fi

# Push the current branch of every submodule, then the workspace (design -> webui -> vizier order, so pins never point at unpushed commits).
# A submodule on a detached HEAD is skipped.
push:
  #!/usr/bin/env bash
  set -euo pipefail
  for sm in $(git submodule --quiet foreach 'echo $sm_path'); do
    if git -C "$sm" symbolic-ref -q HEAD >/dev/null; then
      echo -e "\e[1;34m$sm\e[0m"
      git -C "$sm" push -u origin HEAD
    else
      echo -e "\e[1;33m$sm: detached HEAD, skipped\e[0m"
    fi
  done
  git push -u origin HEAD

# Delete <branch> in the workspace and every submodule that has it.
# Safe by default: only merged branches, local only. Flags: --force (delete unmerged too), --remote (also delete it on origin).
branch-rm branch *flags:
  #!/usr/bin/env bash
  set -euo pipefail
  del=-d; remote=0
  for f in {{flags}}; do
    case "$f" in
      --force) del=-D ;;
      --remote) remote=1 ;;
      *) echo "unknown flag $f (use --force, --remote)"; exit 1 ;;
    esac
  done
  for repo in . $(git submodule --quiet foreach 'echo $sm_path'); do
    (
      cd "$repo"
      if git show-ref --verify --quiet "refs/heads/{{branch}}"; then
        git branch $del "{{branch}}" && echo "$repo: deleted local {{branch}}" || echo -e "\e[1;33m$repo: kept {{branch}} (checked out, or unmerged: use --force)\e[0m"
      fi
      if [ "$remote" = 1 ] && git ls-remote --exit-code --heads origin "{{branch}}" >/dev/null 2>&1; then
        git push origin --delete "{{branch}}" && echo "$repo: deleted origin/{{branch}}"
      fi
    )
  done

# A sibling worktree ../vizier-<branch> (slashes become dashes) on <branch>, created if missing, with submodules on the same branch.
# Dependencies are not shared: run `just install` there (or `npm i` in the pieces you touch).
worktree branch:
  #!/usr/bin/env bash
  set -euo pipefail
  dir="../vizier-$(echo "{{branch}}" | tr '/' '-')"
  git fetch --all --prune
  if git show-ref --verify --quiet "refs/heads/{{branch}}" || git show-ref --verify --quiet "refs/remotes/origin/{{branch}}"; then
    git worktree add "$dir" "{{branch}}"
  else
    git worktree add -b "{{branch}}" "$dir"
  fi
  git -C "$dir" submodule update --init
  (cd "$dir" && just checkout "{{branch}}")
  echo -e "\n\e[1;32mworktree ready: $(cd "$dir" && pwd)\e[0m"

# Every worktree of this repository.
worktrees:
  @git worktree list

# Remove the worktree made by `just worktree <branch>`. Refuses while it or its submodules have uncommitted changes; the branch is kept.
worktree-rm branch:
  #!/usr/bin/env bash
  set -euo pipefail
  dir="../vizier-$(echo "{{branch}}" | tr '/' '-')"
  [ -d "$dir" ] || { echo "no worktree at $dir"; exit 1; }
  if [ -n "$(git -C "$dir" status --porcelain)" ] || [ -n "$(git -C "$dir" submodule foreach --quiet --recursive 'git status --porcelain')" ]; then
    echo -e "\e[1;31m$dir has uncommitted changes, not removing\e[0m"
    exit 1
  fi
  # git refuses to remove a worktree that has submodules without --force; it is clean, so that is safe
  git worktree remove --force "$dir"
  git worktree prune
  echo "removed $dir (branch {{branch}} kept)"

test:
  cargo test --workspace

lint:
  cargo clippy --workspace

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

app-dev:
  cd app && cargo tauri dev

app-build:
  cd webui && npm run build
  cd app && cargo tauri build

app-test:
  cd app && cargo test
