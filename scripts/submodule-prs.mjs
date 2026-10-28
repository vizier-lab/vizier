#!/usr/bin/env node
/**
 * Links a vizier pull request to its pull requests in the submodule
 * repositories (`design/`, `webui/`, `dummyplug/`) and merges them in dependency order.
 *
 * Two commands:
 *
 *   node scripts/submodule-prs.mjs link --pr <n>
 *       Detects the linked submodule PRs, keeps a sticky comment on the vizier
 *       PR up to date, and reports a `submodule-prs` commit status. Never
 *       writes to the submodule repositories.
 *
 *   node scripts/submodule-prs.mjs cascade --pr <n> [--dry-run]
 *       Merges the submodule PRs (design before webui; dummyplug before vizier), bumps the vizier
 *       gitlinks to the commits that landed, pushes that to the PR branch, and
 *       then merges the vizier PR. This is the bottom-up order the gitlinks
 *       need: a vizier pin must reference a commit that is on the submodule's
 *       default branch, which a squash merge only makes true after the merge.
 *
 * Links are found from, and combined across: a `Submodule-PRs:` block in the
 * vizier PR body, the PR's branch name (the same branch in a submodule the PR
 * pins), and the gitlinks the PR changes (a submodule PR whose head is that
 * commit).
 *
 * Everything cross-repository needs a token that can act on the submodule
 * repositories: `GH_TOKEN` must be a PAT or GitHub App token with contents and
 * pull-requests write on vizier-lab/design, vizier-lab/webui, and vizier-lab/dummyplug.
 */

import { execFileSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';

export const MARKER_LINKS = '<!-- vizier-submodule-links -->';
export const MARKER_CASCADE = '<!-- vizier-submodule-cascade -->';
export const STATUS_CONTEXT = 'submodule-prs';

/** Dependency order: webui imports design; Vizier pins its standalone dummyplug server. */
export const PRIORITY = ['design', 'webui', 'dummyplug'];

/** Conclusions that mean a check did not pass. */
const FAILED_CONCLUSIONS = new Set([
  'FAILURE',
  'ERROR',
  'CANCELLED',
  'TIMED_OUT',
  'ACTION_REQUIRED',
  'STARTUP_FAILURE',
]);

// ---------------------------------------------------------------------------
// Pure helpers (unit-tested in scripts/submodule-prs.test.mjs)
// ---------------------------------------------------------------------------

/** `owner/repo` for a GitHub remote URL, in either https or ssh form. */
export function repoFromUrl(url) {
  const m = /github\.com[:/]([^/]+)\/([^/]+?)(?:\.git)?\/?$/.exec(String(url || '').trim());
  return m ? `${m[1]}/${m[2]}` : '';
}

/** Parse `git config -f .gitmodules --get-regexp` output into submodule entries. */
export function parseSubmoduleList(text) {
  const entries = new Map();
  for (const raw of String(text || '').split('\n')) {
    const m = /^submodule\.(.+)\.(path|url)\s+(.+)$/.exec(raw.trim());
    if (!m) continue;
    const [, name, key, value] = m;
    const entry = entries.get(name) || { name, path: '', repo: '' };
    if (key === 'path') entry.path = value.trim();
    if (key === 'url') entry.repo = repoFromUrl(value);
    entries.set(name, entry);
  }
  return [...entries.values()].filter((e) => e.path && e.repo);
}

/** Order submodules so a dependency is merged before the things that use it. */
export function orderSubmodules(submodules) {
  const rank = (sub) => {
    const i = PRIORITY.indexOf(sub.name);
    return i === -1 ? PRIORITY.length : i;
  };
  return [...submodules].sort((a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name));
}

/**
 * The `Submodule-PRs:` block from a PR body:
 *
 *   Submodule-PRs: vizier-lab/design#12, vizier-lab/webui#34
 *
 * or the same as a list, one per line. Only fully qualified `owner/repo#n`
 * references count; a bare `#12` is ignored (it could mean any repository).
 */
export function parseMarkerRefs(body) {
  // HTML comments are instructions, not links: a commented-out example in the
  // PR template must not link anything.
  const lines = String(body || '')
    .replace(/<!--[\s\S]*?-->/g, '')
    .split(/\r?\n/);
  const refs = [];
  let inBlock = false;
  for (const line of lines) {
    const header = /^\s*submodule[- ]?prs?\s*:\s*(.*)$/i.exec(line);
    if (header) {
      inBlock = true;
      collect(header[1]);
      continue;
    }
    if (inBlock) {
      if (!line.trim() || !/#\d+/.test(line)) {
        inBlock = false;
        continue;
      }
      collect(line);
    }
  }
  return refs;

  function collect(text) {
    for (const m of String(text).replace(/`/g, '').matchAll(/([\w.-]+\/[\w.-]+)#(\d+)/g)) {
      refs.push({ repo: m[1], number: Number(m[2]), source: 'marker' });
    }
  }
}

/** Parse `git diff --raw` output into the gitlinks it changes. */
export function parseGitlinkDiff(text) {
  const links = [];
  for (const raw of String(text || '').split('\n')) {
    const m = /^:(\d{6}) (\d{6}) ([0-9a-f]{40}) ([0-9a-f]{40}) ([A-Z])\t(.+)$/.exec(raw.trim());
    if (!m) continue;
    const [, oldMode, newMode, oldSha, newSha, , path] = m;
    if (oldMode !== '160000' && newMode !== '160000') continue;
    links.push({
      path,
      oldSha: oldMode === '160000' ? oldSha : null,
      newSha: newMode === '160000' && !/^0+$/.test(newSha) ? newSha : null,
    });
  }
  return links;
}

/** Dedupe targets by `repo#number`, keeping the first source and all sources. */
export function dedupeTargets(targets) {
  const byKey = new Map();
  for (const target of targets) {
    const key = `${target.repo}#${target.number}`;
    const existing = byKey.get(key);
    if (existing) {
      if (target.source && !existing.sources.includes(target.source)) {
        existing.sources.push(target.source);
      }
      continue;
    }
    byKey.set(key, { ...target, sources: target.source ? [target.source] : [] });
  }
  return [...byKey.values()];
}

/** Sort targets by the dependency order of the repository they belong to. */
export function orderTargets(targets, submodules) {
  const rank = new Map(orderSubmodules(submodules).map((s, i) => [s.repo, i]));
  return [...targets].sort(
    (a, b) =>
      (rank.get(a.repo) ?? submodules.length) - (rank.get(b.repo) ?? submodules.length) ||
      a.number - b.number,
  );
}

/** True for a `/cascade-merge` comment (optionally with `--dry-run`). */
export function isCascadeCommand(body) {
  return /^\s*\/cascade-merge\b/i.test(String(body || ''));
}

/** Whether a commenter may drive the cascade. */
export function mayCascade(association) {
  return ['OWNER', 'MEMBER', 'COLLABORATOR'].includes(String(association || '').toUpperCase());
}

/** Failed checks in a `statusCheckRollup`, ignoring passing and pending ones. */
export function failedChecks(rollup) {
  return (rollup || []).filter((check) =>
    FAILED_CONCLUSIONS.has(String(check.conclusion || check.state || '').toUpperCase()),
  );
}

/** Whether every check in a rollup has finished. */
export function checksFinished(rollup) {
  return (rollup || []).every((check) => {
    const state = String(check.state || '').toUpperCase();
    const conclusion = String(check.conclusion || '').toUpperCase();
    if (state) return state !== 'PENDING' && state !== 'EXPECTED';
    return Boolean(conclusion);
  });
}

/** The body of the sticky comment the `link` command maintains. */
export function renderLinksComment({ targets, gitlinks, warnings, cascadeOrder }) {
  const lines = [];
  lines.push('### Submodule PRs');
  lines.push('');
  if (targets.length === 0) {
    lines.push(
      gitlinks.length === 0
        ? 'This PR changes no submodule gitlinks, so there is nothing to link.'
        : 'This PR changes submodule gitlinks but no matching PR was found. Add a `Submodule-PRs:` line to the PR body if you want them merged together.',
    );
  } else {
    lines.push('| Submodule | PR | State | Vizier pins | PR head |');
    lines.push('| --- | --- | --- | --- | --- |');
    for (const target of targets) {
      const pin = gitlinks.find((g) => g.repo === target.repo)?.newSha;
      const head = target.headRefOid;
      const pinCell = pin ? `\`${pin.slice(0, 7)}\`` : '—';
      const headCell = head
        ? `\`${head.slice(0, 7)}\`${pin && pin !== head ? ' (bumped by the cascade)' : ''}`
        : '—';
      lines.push(
        `| ${target.name || target.repo.split('/')[1]} | ${prLink(target)} | ${stateText(target)} | ${pinCell} | ${headCell} |`,
      );
    }
    lines.push('');
    lines.push(`Linked by: ${[...new Set(targets.flatMap((t) => t.sources))].map(sourceText).join(', ')}.`);
    lines.push('');
    if (cascadeOrder.length) {
      lines.push(
        `Merging happens bottom-up, so the gitlinks in this PR point at commits on the submodules' default branches: comment \`/cascade-merge\` to merge ${cascadeOrder
          .map((t) => prRef(t))
          .join(' → ')} → this PR.`,
      );
    } else {
      lines.push('Every linked submodule PR is already merged.');
    }
  }
  for (const warning of warnings || []) {
    lines.push('', `> [!WARNING]`, `> ${warning}`);
  }
  lines.push('', '<sub>Updated by `scripts/submodule-prs.mjs`.</sub>');
  return lines.join('\n');
}

function prLink(target) {
  return target.url ? `[#${target.number}](${target.url})` : `#${target.number}`;
}

function prRef(target) {
  return target.url ? `[${target.repo}#${target.number}](${target.url})` : `${target.repo}#${target.number}`;
}

function stateText(target) {
  if (target.missing) return '**unreadable**';
  if (target.state === 'MERGED') return 'merged';
  if (target.state === 'CLOSED') return '**closed without merging**';
  if (target.isDraft) return 'open · draft';
  if (target.mergeable === 'CONFLICTING') return 'open · **conflicts**';
  return 'open';
}

function sourceText(source) {
  return { marker: '`Submodule-PRs:` in the body', branch: 'branch name', sha: 'gitlink commit' }[source] || source;
}

const sleep = (seconds) => Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, seconds * 1000);

function waitFor(label, poll, { timeoutMs = 20 * 60_000, intervalMs = 10_000, log = () => {} } = {}) {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const value = poll();
    if (value) return value;
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${label}`);
    log(`waiting for ${label}…`);
    sleep(intervalMs / 1000);
  }
}

// ---------------------------------------------------------------------------
// GitHub / git access
// ---------------------------------------------------------------------------

function ensureGh() {
  try {
    execFileSync('gh', ['--version'], { stdio: 'ignore' });
  } catch {
    throw new Error('the GitHub CLI (`gh`) is required');
  }
}

function gh(args, { input, allowFail = false } = {}) {
  try {
    return execFileSync('gh', args, {
      encoding: 'utf8',
      input,
      env: process.env,
      stdio: ['pipe', 'pipe', 'pipe'],
      maxBuffer: 32 * 1024 * 1024,
    }).trim();
  } catch (error) {
    if (allowFail) return null;
    const stderr = String(error.stderr || error.message).trim();
    throw new Error(`gh ${args.join(' ')} failed: ${stderr}`);
  }
}

function ghJson(args, options) {
  const out = gh(args, options);
  return out ? JSON.parse(out) : null;
}

function git(args, { allowFail = false } = {}) {
  try {
    return execFileSync('git', args, {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
      maxBuffer: 16 * 1024 * 1024,
    }).trim();
  } catch (error) {
    if (allowFail) return null;
    throw new Error(`git ${args.join(' ')} failed: ${String(error.stderr || error.message).trim()}`);
  }
}

function readSubmodules() {
  const out = git(['config', '-f', '.gitmodules', '--get-regexp', '^submodule\\..*\\.(path|url)$'], {
    allowFail: true,
  });
  return parseSubmoduleList(out || '');
}

function changedGitlinks(base) {
  git(['fetch', '--quiet', 'origin', base], { allowFail: true });
  const range = git(['rev-parse', '--verify', '--quiet', `origin/${base}`], { allowFail: true })
    ? `origin/${base}...HEAD`
    : `HEAD^...HEAD`;
  // `--no-abbrev` matters: raw diffs otherwise print 7-character object names.
  return parseGitlinkDiff(git(['diff', '--raw', '--no-abbrev', range], { allowFail: true }) || '');
}

function viewPr(repo, number) {
  return ghJson([
    'pr',
    'view',
    String(number),
    '-R',
    repo,
    '--json',
    'number,title,url,body,state,isDraft,mergeable,mergeStateStatus,baseRefName,headRefName,headRefOid,headRepository,headRepositoryOwner,mergeCommit,statusCheckRollup',
  ]);
}

function defaultBranch(repo) {
  return ghJson(['repo', 'view', repo, '--json', 'defaultBranchRef'])?.defaultBranchRef?.name || 'master';
}

// ---------------------------------------------------------------------------
// Link detection
// ---------------------------------------------------------------------------

/** Assemble the linked targets from the body, the branch name and the gitlinks. */
export function collectTargets({ body, headRefName, gitlinks, submodules, findBranchPrs, findShaPrs }) {
  const warnings = [];
  const targets = [];

  // 1. Explicit marker in the body (authoritative, may name any submodule).
  for (const ref of parseMarkerRefs(body)) {
    const submodule = submodules.find((s) => s.repo.toLowerCase() === ref.repo.toLowerCase());
    if (!submodule) {
      warnings.push(
        `\`Submodule-PRs:\` names \`${ref.repo}\`, which is not a submodule in \`.gitmodules\`; it was ignored.`,
      );
      continue;
    }
    targets.push({ ...ref, repo: submodule.repo, name: submodule.name });
  }

  // 2. The same branch in a submodule this PR changes (the usual convention).
  //    Restricted to changed gitlinks: a same-named branch elsewhere is not
  //    evidence of anything, and merging it would be a surprise.
  if (headRefName) {
    for (const submodule of submodules) {
      if (!gitlinks.some((link) => link.repo === submodule.repo)) continue;
      for (const foundPr of findBranchPrs(submodule) || []) {
        if (foundPr.headRefName !== headRefName) continue;
        targets.push({ repo: submodule.repo, name: submodule.name, number: foundPr.number, source: 'branch' });
      }
    }
  }

  // 3. A submodule PR whose head is the commit this PR pins.
  for (const link of gitlinks) {
    if (!link.repo || !link.newSha) continue;
    for (const foundPr of findShaPrs(link) || []) {
      if (foundPr.state !== 'open') continue;
      targets.push({
        repo: link.repo,
        name: submodules.find((s) => s.repo === link.repo)?.name,
        number: foundPr.number,
        source: 'sha',
      });
    }
  }

  return { targets: orderTargets(dedupeTargets(targets), submodules), warnings };
}

export function resolveLinks({ repo, pr }) {
  const submodules = readSubmodules();
  const warnings = [];

  const pull = viewPr(repo, pr);
  const base = pull.baseRefName || defaultBranch(repo);
  const gitlinks = changedGitlinks(base).map((link) => ({
    ...link,
    repo: submodules.find((s) => s.path === link.path)?.repo || '',
  }));

  const collected = collectTargets({
    body: pull.body,
    headRefName: pull.headRefName,
    gitlinks,
    submodules,
    findBranchPrs: (submodule) =>
      ghJson(
        [
          'pr',
          'list',
          '-R',
          submodule.repo,
          '--state',
          'open',
          '--head',
          pull.headRefName,
          '--json',
          'number,url,headRefName,isDraft,headRefOid,mergeable,mergeStateStatus,state,baseRefName',
        ],
        { allowFail: true },
      ),
    findShaPrs: (link) => ghJson(['api', `repos/${link.repo}/commits/${link.newSha}/pulls`], { allowFail: true }),
  });
  warnings.push(...collected.warnings);

  // Enrich and verify: every marker/branch/sha target must exist and be open or merged.
  const enriched = [];
  for (const target of collected.targets) {
    const detail = ghJson(['pr', 'view', String(target.number), '-R', target.repo, '--json', 'number,title,url,state,isDraft,mergeable,mergeStateStatus,baseRefName,headRefName,headRefOid,mergeCommit,statusCheckRollup'], {
      allowFail: true,
    });
    if (!detail) {
      warnings.push(`\`${target.repo}#${target.number}\` could not be read (missing PR, or the token cannot see that repository).`);
      enriched.push({ ...target, missing: true });
      continue;
    }
    if (detail.state === 'CLOSED') {
      warnings.push(`\`${target.repo}#${target.number}\` is closed without being merged.`);
    }
    enriched.push({ ...target, ...detail, sources: target.sources });
  }

  return { pr: pull, submodules, gitlinks, targets: enriched, warnings };
}

// ---------------------------------------------------------------------------
// Comment / status plumbing
// ---------------------------------------------------------------------------

function upsertComment({ repo, number, marker, body, dryRun, log }) {
  const full = `${marker}\n${body}`;
  const comments = ghJson(['api', `repos/${repo}/issues/${number}/comments?per_page=100`], { allowFail: true }) || [];
  const existing = comments.find((comment) => String(comment.body || '').includes(marker));
  if (existing && String(existing.body).trim() === full.trim()) {
    log('comment is up to date');
    return;
  }
  if (dryRun) {
    log(`would ${existing ? 'update' : 'post'} the comment:\n\n${full}`);
    return;
  }
  const payload = JSON.stringify({ body: full });
  if (existing) {
    gh(['api', '-X', 'PATCH', `repos/${repo}/issues/comments/${existing.id}`, '--input', '-'], { input: payload });
    log('comment updated');
  } else {
    gh(['api', '-X', 'POST', `repos/${repo}/issues/${number}/comments`, '--input', '-'], { input: payload });
    log('comment posted');
  }
}

function setStatus({ repo, sha, state, description, runUrl, dryRun, log }) {
  if (!sha) return;
  const targetUrl = runUrl || `https://github.com/${repo}/actions`;
  if (dryRun) {
    log(`would set status ${STATUS_CONTEXT}=${state}: ${description}`);
    return;
  }
  gh(
    [
      'api',
      '-X',
      'POST',
      `repos/${repo}/statuses/${sha}`,
      '-f',
      `state=${state}`,
      '-f',
      `context=${STATUS_CONTEXT}`,
      '-f',
      `description=${description.slice(0, 140)}`,
      '-f',
      `target_url=${targetUrl}`,
    ],
    { allowFail: true },
  );
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

function cmdLink({ repo, pr, sha, runUrl, dryRun, log }) {
  const result = resolveLinks({ repo, pr });
  if (result.targets.length === 0 && result.gitlinks.length > 0 && !dryRun) {
    log('no linked submodule PRs found for a PR that changes gitlinks');
  }

  const body = renderLinksComment({
    targets: result.targets,
    gitlinks: result.gitlinks,
    warnings: result.warnings,
    cascadeOrder: result.targets.filter((t) => t.state !== 'MERGED'),
  });
  upsertComment({ repo, number: pr, marker: MARKER_LINKS, body, dryRun, log });

  const broken = result.targets.filter((t) => t.missing || t.state === 'CLOSED');
  const state = broken.length === 0 ? 'success' : 'failure';
  const description = broken.length
    ? `${broken.length} linked submodule PR(s) unusable`
    : result.targets.length
      ? `${result.targets.length} linked submodule PR(s)`
      : result.gitlinks.length
        ? 'gitlinks changed, no linked submodule PRs'
        : 'no submodule gitlinks changed';
  log(`status ${state}: ${description}`);
  setStatus({ repo, sha: sha || result.pr.headRefOid, state, description, runUrl, dryRun, log });

  if (broken.length && !dryRun) process.exitCode = 1;
  return result;
}

async function cmdCascade({ repo, pr, runUrl, dryRun, log, method, deleteBranch }) {
  const { pr: pull, gitlinks, targets } = resolveLinks({ repo, pr });
  const notes = [];

  const fail = (message) => {
    throw new Error(message);
  };

  // --- preflight ----------------------------------------------------------
  if (pull.state !== 'OPEN') fail(`#${pr} is ${pull.state.toLowerCase()}, nothing to do.`);
  if (pull.isDraft) fail(`#${pr} is a draft; mark it ready for review first.`);
  const headOwner = pull.headRepositoryOwner?.login;
  const [baseOwner] = repo.split('/');
  if (headOwner && headOwner.toLowerCase() !== baseOwner.toLowerCase()) {
    fail(`#${pr} comes from a fork (${headOwner}); the cascade needs a branch in ${repo}.`);
  }
  if (targets.length === 0) fail(`no linked submodule PRs found for #${pr}.`);

  const open = targets.filter((t) => t.state !== 'MERGED');
  for (const target of open) {
    if (target.missing) fail(`${target.repo}#${target.number} could not be read.`);
    if (target.state === 'CLOSED') fail(`${target.repo}#${target.number} is closed without being merged.`);
    if (target.baseRefName !== defaultBranch(target.repo)) {
      fail(`${target.repo}#${target.number} targets ${target.baseRefName}, not the default branch.`);
    }
    if (target.isDraft) {
      log(`marking ${target.repo}#${target.number} ready for review`);
      if (!dryRun) gh(['pr', 'ready', String(target.number), '-R', target.repo]);
    }
  }

  const waitOpts = { timeoutMs: Number(process.env.CASCADE_CHECK_TIMEOUT_SECS || 1200) * 1000, log };
  for (const target of open) {
    waitFor(
      `${target.repo}#${target.number} to be mergeable`,
      () => {
        const detail = viewPr(target.repo, target.number);
        const failures = failedChecks(detail.statusCheckRollup);
        if (failures.length) {
          fail(
            `${target.repo}#${target.number} has failing checks: ${failures
              .map((c) => c.name || c.context)
              .join(', ')}.`,
          );
        }
        if (detail.mergeable === 'CONFLICTING') fail(`${target.repo}#${target.number} has conflicts.`);
        const settled = detail.mergeStateStatus && !['UNKNOWN', 'BLOCKED', 'DRAFT'].includes(detail.mergeStateStatus);
        return detail.mergeable === 'MERGEABLE' && settled ? detail : null;
      },
      waitOpts,
    );
  }

  const plan = targets.map((t) => `${t.repo}#${t.number}`);
  log(`cascade plan: ${plan.join(' → ')} → ${repo}#${pr} (${method})`);
  if (dryRun) {
    const body = `**Dry run.** Would merge ${plan
      .map((ref) => `\`${ref}\``)
      .join(' → ')} → \`${repo}#${pr}\`, bumping \`${gitlinks
      .filter((g) => g.repo)
      .map((g) => g.path)
      .join('` and `')}\` to the merged commits.`;
    upsertComment({ repo, number: pr, marker: MARKER_CASCADE, body, dryRun, log });
    return { plan };
  }

  // --- merge the submodules, bottom-up ------------------------------------
  const pins = new Map(); // path -> sha
  for (const target of targets) {
    let merged = target.state === 'MERGED' ? target : null;
    if (!merged) {
      log(`merging ${target.repo}#${target.number}`);
      const args = ['pr', 'merge', String(target.number), '-R', target.repo, `--${method}`];
      if (deleteBranch) args.push('--delete-branch');
      gh(args);
      merged = waitFor(`${target.repo}#${target.number} to be merged`, () => {
        const detail = viewPr(target.repo, target.number);
        return detail.state === 'MERGED' && detail.mergeCommit?.oid ? detail : null;
      }, { timeoutMs: 10 * 60_000, log });
    }
    const commit = merged.mergeCommit?.oid;
    if (!commit) fail(`${target.repo}#${target.number} has no merge commit.`);
    const link = gitlinks.find((g) => g.repo === target.repo);
    if (!link) {
      notes.push(`${target.repo}#${target.number} merged, but this PR pins no \`${target.repo.split('/')[1]}\` gitlink.`);
      continue;
    }
    pins.set(link.path, commit);
    log(`${target.repo}#${target.number} merged as ${commit.slice(0, 7)}`);
  }

  // --- bump the gitlinks --------------------------------------------------
  const bumps = [];
  for (const [path, commit] of pins) {
    const current = git(['rev-parse', `HEAD:${path}`]);
    if (current === commit) continue;
    git(['update-index', '--cacheinfo', `160000,${commit},${path}`]);
    bumps.push(`${path} ${commit.slice(0, 7)}`);
  }

  if (bumps.length) {
    const names = [...pins.keys()].map((path) => path.replace(/\/$/, ''));
    git(['commit', '-m', `chore: bump ${names.join(' and ')} to their merged PRs`]);
    const branch = pull.headRefName;
    log(`pushing the bump to ${branch}`);
    git(['push', 'origin', `HEAD:${branch}`]);
  } else {
    log('gitlinks already point at the merged commits');
  }

  // --- merge the vizier PR ------------------------------------------------
  const localHead = git(['rev-parse', 'HEAD']);
  waitFor(
    `${repo}#${pr} to be mergeable`,
    () => {
      const detail = viewPr(repo, pr);
      // The push above changes the head; do not trust a rollup computed for the old one.
      if (detail.headRefOid !== localHead) return null;
      const failures = failedChecks(detail.statusCheckRollup);
      if (failures.length) {
        fail(`${repo}#${pr} has failing checks after the bump: ${failures.map((c) => c.name || c.context).join(', ')}.`);
      }
      if (detail.mergeable === 'CONFLICTING') fail(`${repo}#${pr} has conflicts.`);
      const settled = detail.mergeStateStatus && !['UNKNOWN', 'BLOCKED', 'DRAFT'].includes(detail.mergeStateStatus);
      return detail.mergeable === 'MERGEABLE' && settled && checksFinished(detail.statusCheckRollup)
        ? detail
        : null;
    },
    waitOpts,
  );

  log(`merging ${repo}#${pr}`);
  const args = ['pr', 'merge', String(pr), '-R', repo, `--${method}`];
  if (deleteBranch) args.push('--delete-branch');
  gh(args);

  // A merge queue answers the merge request before the PR has merged.
  try {
    waitFor(
      `${repo}#${pr} to land`,
      () => viewPr(repo, pr).state === 'MERGED',
      { timeoutMs: 5 * 60_000, log },
    );
  } catch {
    log(`${repo}#${pr} is still queued; it merges when the queue lets it.`);
  }

  const landed = viewPr(repo, pr).state === 'MERGED';
  const body = `### Submodule cascade\n\n${landed ? 'Merged' : 'Queued'} ${plan
    .map((ref) => `\`${ref}\``)
    .join(' → ')} → \`${repo}#${pr}\`.\n\n${bumps.length ? `Bumped: ${bumps.join(', ')}.` : 'Gitlinks were already up to date.'}${
    notes.length ? `\n\n${notes.join('\n\n')}` : ''
  }`;
  upsertComment({ repo, number: pr, marker: MARKER_CASCADE, body, dryRun, log });
  setStatus({
    repo,
    sha: localHead,
    state: 'success',
    description: landed ? 'submodules merged' : 'submodules merged, PR queued',
    runUrl,
    dryRun,
    log,
  });
  return { plan, bumps };
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

function argValue(argv, name) {
  const index = argv.indexOf(name);
  return index === -1 ? undefined : argv[index + 1];
}

function hasFlag(argv, name) {
  return argv.includes(name);
}

export async function main(argv = process.argv.slice(2)) {
  ensureGh();
  const [command] = argv;
  const repo = process.env.GITHUB_REPOSITORY;
  const pr = Number(argValue(argv, '--pr') || process.env.PR_NUMBER);
  const dryRun = hasFlag(argv, '--dry-run') || process.env.DRY_RUN === 'true';
  const sha = process.env.HEAD_SHA || undefined;
  const runUrl = process.env.RUN_URL || undefined;
  const method = (process.env.CASCADE_MERGE_METHOD || 'squash').toLowerCase();
  const deleteBranch = process.env.CASCADE_DELETE_BRANCH !== 'false';
  const log = (message) => process.stderr.write(`${message}\n`);

  if (!repo || !pr) {
    throw new Error('GITHUB_REPOSITORY and --pr (or PR_NUMBER) are required');
  }
  if (!['squash', 'merge', 'rebase'].includes(method)) {
    throw new Error(`unsupported CASCADE_MERGE_METHOD: ${method}`);
  }

  if (command === 'link') {
    return cmdLink({ repo, pr, sha, runUrl, dryRun, log });
  }
  if (command === 'cascade') {
    const comment = process.env.COMMENT_BODY;
    if (String(comment || '').trim() && !isCascadeCommand(comment)) {
      throw new Error('the cascade is driven by a `/cascade-merge` comment');
    }
    const association = process.env.COMMENT_AUTHOR_ASSOCIATION;
    if (association && !mayCascade(association)) {
      throw new Error(`only owners, members and collaborators may run the cascade (got ${association})`);
    }
    return cmdCascade({ repo, pr, runUrl, dryRun, log, method, deleteBranch });
  }
  throw new Error(`unknown command: ${command} (expected link or cascade)`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`\n${error.message}\n`);
    process.exitCode = 1;
  });
}
