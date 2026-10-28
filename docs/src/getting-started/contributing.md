# Building from Source

Vizier is one Rust binary that embeds its WebUI. The WebUI and the component library it is built from live in their own repositories, which a vizier checkout pulls in as git submodules:

| Path | Repository | What it is |
|------|------------|------------|
| `webui/` | `vizier-lab/webui` | The WebUI (React Router, built to `webui/build/client/`) |
| `design/` | `vizier-lab/design` | The component library and its Storybook catalogue |
| `landing/` | `vizier-lab/landing` | The website: landing page, blog and docs (Astro + Starlight) |
| `legacy/` | (this repository) | The previous WebUI, kept for reference. It is never built, served or packaged |

## First build

```sh
git clone --recurse-submodules https://github.com/vizier-lab/vizier.git
cd vizier
just install     # submodules, npm install in design/, webui/ and landing/, cargo fetch
just run         # builds the WebUI, then runs vizier with dev.vizier.yaml
just landing     # the website on http://localhost:4321 (docs + design from this checkout)
```

If you cloned without `--recurse-submodules`, run `git submodule update --init --recursive`. A `cargo build` with no `webui/node_modules/` and no pre-built `webui/build/client/` stops with a message saying so.

## Working on the website

Docs are authored in `docs/src/`, next to the code. The website syncs them at
build time; `just landing` starts it on http://localhost:4321. Blog posts live
in `landing/src/content/blog/`. The site is deployed to www.vizier.rs from the
[`landing`](https://github.com/vizier-lab/landing) repository, and a vizier
release dispatches a deploy with the release tag.

The design system's Storybook catalogue is published with the same site, at
[www.vizier.rs/design](https://www.vizier.rs/design). A push to `design` master
dispatches a landing deploy that rebuilds the catalogue from that commit, so the
catalogue tracks the current design rather than vizier's submodule pin.

## Working on the WebUI

```sh
just run                    # vizier on :9999
cd webui && npm run dev     # the WebUI on http://localhost:5173, proxying /api to :9999
```

A change that needs a new or changed component is made in `design/` first. Commit it there, then commit the screen in `webui/`, then commit vizier with both submodule pins bumped.

## Linking submodule pull requests

A vizier PR that bumps `webui/` or `design/` usually has a PR open in those repositories. Link them and one command merges everything in dependency order.

Any of these links a PR; all are collected:

- a `Submodule-PRs:` block in the vizier PR body (explicit, and the only way to link a submodule this PR does not pin):

  ```
  Submodule-PRs: vizier-lab/design#12, vizier-lab/webui#34
  ```

  A list under the header works too.
- the same branch name in `design/` or `webui/`, when the PR changes that submodule's gitlink (the usual convention: one branch name across all three repositories)
- a submodule PR whose head commit is the commit the vizier PR pins

A **Submodule PRs** comment stays up to date on the vizier PR, and a `submodule-prs` commit status reports whether the linked PRs are usable — add it as a required check if it should gate merges.

### Merging: `/cascade-merge`

Comment `/cascade-merge` on the vizier PR (owners, members and collaborators). It:

1. preflights every linked PR — open, not a draft, targeting the default branch, mergeable, required checks green — before touching anything;
2. merges them in dependency order, `design` then `webui`;
3. bumps the gitlinks in the vizier PR branch to the commits that landed, and pushes that commit;
4. waits for the vizier PR's checks, then merges it.

`/cascade-merge --dry-run` reports the plan and merges nothing. A failure during preflight merges nothing; a failure after a submodule PR merged leaves that merge in place — fix the cause and comment `/cascade-merge` again, the already-merged PRs are skipped.

The order is deliberate: a vizier gitlink must name a commit on the submodule's default branch. Merging the vizier PR first would pin a commit that lives only on the submodule PR's branch, and a squash merge never makes it an ancestor of `master`, so a later `git submodule update` could not fetch it.

### Setup

Cross-repository work needs a token that can merge PRs in `vizier-lab/design` and `vizier-lab/webui`: add a repository secret `SUBMODULE_TOKEN` (a fine-grained PAT or GitHub App token with **Contents** and **Pull requests** read/write on both repositories). Without it, only the linking comment works, using `GITHUB_TOKEN`.

Optional repository variables:

| Variable | Default | What it does |
| --- | --- | --- |
| `CASCADE_MERGE_METHOD` | `squash` | `squash`, `merge` or `rebase` for both the submodule and vizier PRs |
| `CASCADE_DELETE_BRANCH` | `true` | Delete each PR's branch after merging |
| `CASCADE_CHECK_TIMEOUT_SECS` | `1200` | How long to wait for checks before giving up |

The detection and planning logic is unit-tested: `just test-scripts` (or `node --test scripts/*.test.mjs`).

## Releasing

Releases are dispatched from the **Release, Build & Publish** workflow with a
version (`0.13.3`, `0.14.0-rc.1`). It bumps the workspace version, refreshes the
model catalogue, regenerates `CHANGELOG.md`, commits the bump to `master` and tags
it, then builds the binaries, publishes to crates.io and the container registries,
and dispatches the website deploy. `dry_run: true` runs everything except the push,
the tag, the GitHub release and the publishes.

`master` requires a pull request, so the bump commit is pushed with a token that may
bypass the ruleset. The built-in Actions app cannot be a bypass actor on a
repository ruleset ("Actor GitHub Actions integration must be part of the ruleset
source or owner organization"), so the job reads `RELEASE_TOKEN`: a fine-grained
token owned by a person on the ruleset's bypass list, with **Contents: read and
write** on this repository. Create one under Settings › Developer settings ›
Personal access tokens › Fine-grained tokens, then store it:

```sh
gh secret set RELEASE_TOKEN --repo vizier-lab/vizier
```

Without it the workflow falls back to `GITHUB_TOKEN` and the push to `master` is
rejected with GH013. A failed run deletes its tag and GitHub release, so dispatching
again with the same version is safe.

