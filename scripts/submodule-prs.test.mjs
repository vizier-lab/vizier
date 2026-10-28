import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
  checksFinished,
  collectTargets,
  dedupeTargets,
  failedChecks,
  isCascadeCommand,
  mayCascade,
  orderSubmodules,
  orderTargets,
  parseGitlinkDiff,
  parseMarkerRefs,
  parseSubmoduleList,
  repoFromUrl,
  renderLinksComment,
} from './submodule-prs.mjs';

describe('repoFromUrl', () => {
  it('reads https and ssh GitHub remotes', () => {
    assert.equal(repoFromUrl('https://github.com/vizier-lab/webui.git'), 'vizier-lab/webui');
    assert.equal(repoFromUrl('https://github.com/vizier-lab/design'), 'vizier-lab/design');
    assert.equal(repoFromUrl('git@github.com:vizier-lab/design.git'), 'vizier-lab/design');
    assert.equal(repoFromUrl('https://gitlab.com/a/b.git'), '');
  });
});

describe('parseSubmoduleList', () => {
  it('pairs path and url for each submodule', () => {
    const submodules = parseSubmoduleList(`
submodule.design.path design
submodule.design.url https://github.com/vizier-lab/design.git
submodule.webui.path webui
submodule.webui.url https://github.com/vizier-lab/webui.git
submodule.dummyplug.path dummyplug
submodule.dummyplug.url https://github.com/vizier-lab/dummyplug.git
`);
    assert.deepEqual(submodules, [
      { name: 'design', path: 'design', repo: 'vizier-lab/design' },
      { name: 'webui', path: 'webui', repo: 'vizier-lab/webui' },
      { name: 'dummyplug', path: 'dummyplug', repo: 'vizier-lab/dummyplug' },
    ]);
  });

  it('ignores entries without both fields', () => {
    assert.deepEqual(parseSubmoduleList('submodule.legacy.path legacy'), []);
  });
});

describe('orderSubmodules', () => {
  it('puts a dependency before the submodule that imports it', () => {
    const ordered = orderSubmodules([
      { name: 'dummyplug', path: 'dummyplug' },
      { name: 'webui', path: 'webui' },
      { name: 'design', path: 'design' },
    ]);
    assert.deepEqual(ordered.map((s) => s.name), ['design', 'webui', 'dummyplug']);
  });
});

describe('parseMarkerRefs', () => {
  it('reads a comma-separated line', () => {
    const refs = parseMarkerRefs('Body\n\nSubmodule-PRs: vizier-lab/design#12, vizier-lab/webui#34\n\nMore prose.');
    assert.deepEqual(refs, [
      { repo: 'vizier-lab/design', number: 12, source: 'marker' },
      { repo: 'vizier-lab/webui', number: 34, source: 'marker' },
    ]);
  });

  it('reads a list under the header', () => {
    const refs = parseMarkerRefs('Submodule PRs:\n- vizier-lab/design#12\n- `vizier-lab/webui#34`\n\nUnrelated.');
    assert.deepEqual(refs.map((r) => `${r.repo}#${r.number}`), ['vizier-lab/design#12', 'vizier-lab/webui#34']);
  });

  it('ignores bare numbers and prose without a repository', () => {
    assert.deepEqual(parseMarkerRefs('Submodule-PRs: #12, fixes #13'), []);
  });

  it('returns nothing without a header', () => {
    assert.deepEqual(parseMarkerRefs('This mentions vizier-lab/design#12 in passing.'), []);
  });

  it('ignores a commented-out example from the PR template', () => {
    const body = '## Submodule PRs\n\n<!--\nSubmodule-PRs: vizier-lab/design#12\n-->\n\nCloses #1';
    assert.deepEqual(parseMarkerRefs(body), []);
  });
});

describe('parseGitlinkDiff', () => {
  const sha = 'a'.repeat(40);
  const sha2 = 'b'.repeat(40);

  it('reads changed gitlinks and ignores files', () => {
    const links = parseGitlinkDiff(
      [
        `:160000 160000 ${sha} ${sha2} M\twebui`,
        `:100644 100644 ${sha} ${sha2} M\tsrc/main.rs`,
        `:160000 160000 ${sha} 0000000000000000000000000000000000000000 D\tdesign`,
      ].join('\n'),
    );
    assert.deepEqual(links, [
      { path: 'webui', oldSha: sha, newSha: sha2 },
      { path: 'design', oldSha: sha, newSha: null },
    ]);
  });

  it('returns nothing for a diff without submodules', () => {
    assert.deepEqual(parseGitlinkDiff(`:100644 100644 ${sha} ${sha2} M\tCargo.toml`), []);
  });
});

describe('dedupeTargets', () => {
  it('keeps every source for one PR', () => {
    const targets = dedupeTargets([
      { repo: 'vizier-lab/design', number: 1, source: 'marker' },
      { repo: 'vizier-lab/design', number: 1, source: 'branch' },
      { repo: 'vizier-lab/webui', number: 2, source: 'sha' },
    ]);
    assert.deepEqual(targets, [
      { repo: 'vizier-lab/design', number: 1, source: 'marker', sources: ['marker', 'branch'] },
      { repo: 'vizier-lab/webui', number: 2, source: 'sha', sources: ['sha'] },
    ]);
  });
});

describe('orderTargets', () => {
  it('merges design before webui', () => {
    const submodules = [
      { name: 'webui', repo: 'vizier-lab/webui' },
      { name: 'design', repo: 'vizier-lab/design' },
    ];
    const ordered = orderTargets(
      [
        { repo: 'vizier-lab/webui', number: 2 },
        { repo: 'vizier-lab/design', number: 1 },
      ],
      submodules,
    );
    assert.deepEqual(ordered.map((t) => t.repo), ['vizier-lab/design', 'vizier-lab/webui']);
  });
});

describe('cascade commands', () => {
  it('accepts the command with arguments', () => {
    assert.equal(isCascadeCommand('/cascade-merge'), true);
    assert.equal(isCascadeCommand('/cascade-merge --dry-run'), true);
    assert.equal(isCascadeCommand('/cascade-merge\nthanks'), true);
    assert.equal(isCascadeCommand('/cascade-mergex'), false);
    assert.equal(isCascadeCommand('please /cascade-merge'), false);
  });

  it('allows owners, members and collaborators only', () => {
    assert.equal(mayCascade('OWNER'), true);
    assert.equal(mayCascade('collaborator'), true);
    assert.equal(mayCascade('CONTRIBUTOR'), false);
    assert.equal(mayCascade('NONE'), false);
  });
});

describe('check rollups', () => {
  const rollup = [
    { name: 'build', state: 'SUCCESS', conclusion: 'SUCCESS' },
    { name: 'test', state: 'PENDING', conclusion: '' },
  ];

  it('reports failures, not pending checks', () => {
    assert.deepEqual(failedChecks(rollup), []);
    assert.deepEqual(failedChecks([{ name: 'lint', state: 'FAILURE', conclusion: 'FAILURE' }]).map((c) => c.name), ['lint']);
    assert.deepEqual(failedChecks([{ name: 'doc', state: 'COMPLETED', conclusion: 'ERROR' }]).map((c) => c.name), ['doc']);
  });

  it('knows when nothing is still running', () => {
    assert.equal(checksFinished(rollup), false);
    assert.equal(checksFinished([{ name: 'build', state: 'SUCCESS', conclusion: 'SUCCESS' }]), true);
    assert.equal(checksFinished([]), true);
  });
});

describe('collectTargets', () => {
  const submodules = [
    { name: 'design', path: 'design', repo: 'vizier-lab/design' },
    { name: 'webui', path: 'webui', repo: 'vizier-lab/webui' },
  ];
  const gitlinks = [
    { path: 'design', repo: 'vizier-lab/design', newSha: 'a'.repeat(40) },
    { path: 'webui', repo: 'vizier-lab/webui', newSha: 'b'.repeat(40) },
  ];
  const none = () => [];

  it('takes a marker link and orders it by dependency', () => {
    const { targets } = collectTargets({
      body: 'Submodule-PRs: vizier-lab/webui#34, vizier-lab/design#12',
      headRefName: 'feat/x',
      gitlinks,
      submodules,
      findBranchPrs: none,
      findShaPrs: none,
    });
    assert.deepEqual(targets.map((t) => t.repo), ['vizier-lab/design', 'vizier-lab/webui']);
    assert.deepEqual(targets.map((t) => t.number), [12, 34]);
  });

  it('warns about a marker naming something that is not a submodule', () => {
    const { targets, warnings } = collectTargets({
      body: 'Submodule-PRs: vizier-lab/other#1',
      headRefName: null,
      gitlinks: [],
      submodules,
      findBranchPrs: none,
      findShaPrs: none,
    });
    assert.deepEqual(targets, []);
    assert.match(warnings[0], /not a submodule/);
  });

  it('links by branch name only for a gitlink the PR changes', () => {
    const findBranchPrs = (submodule) =>
      submodule.name === 'webui'
        ? [{ number: 7, headRefName: 'feat/x' }]
        : [{ number: 8, headRefName: 'feat/x' }];
    const { targets } = collectTargets({
      body: '',
      headRefName: 'feat/x',
      gitlinks: [gitlinks[1]],
      submodules,
      findBranchPrs,
      findShaPrs: none,
    });
    assert.deepEqual(targets.map((t) => `${t.repo}#${t.number}`), ['vizier-lab/webui#7']);
  });

  it('links by the pinned commit, ignoring closed PRs', () => {
    const { targets } = collectTargets({
      body: '',
      headRefName: null,
      gitlinks,
      submodules,
      findBranchPrs: none,
      findShaPrs: (link) =>
        link.repo === 'vizier-lab/design'
          ? [
              { number: 3, state: 'open' },
              { number: 4, state: 'closed' },
            ]
          : [],
    });
    assert.deepEqual(targets.map((t) => t.number), [3]);
  });

  it('collects every source that found the same PR', () => {
    const { targets } = collectTargets({
      body: 'Submodule-PRs: vizier-lab/webui#7',
      headRefName: 'feat/x',
      gitlinks,
      submodules,
      findBranchPrs: (submodule) => (submodule.name === 'webui' ? [{ number: 7, headRefName: 'feat/x' }] : []),
      findShaPrs: (link) => (link.repo === 'vizier-lab/webui' ? [{ number: 7, state: 'open' }] : []),
    });
    assert.equal(targets.length, 1);
    assert.deepEqual(targets[0].sources, ['marker', 'branch', 'sha']);
  });
});

describe('renderLinksComment', () => {
  const gitlinks = [{ path: 'webui', repo: 'vizier-lab/webui', newSha: 'b'.repeat(40) }];
  const targets = [
    {
      repo: 'vizier-lab/webui',
      name: 'webui',
      number: 7,
      url: 'https://github.com/vizier-lab/webui/pull/7',
      state: 'OPEN',
      headRefOid: 'a'.repeat(40),
      sources: ['marker', 'branch'],
    },
  ];

  it('shows the link table and the order of the cascade', () => {
    const body = renderLinksComment({ targets, gitlinks, warnings: [], cascadeOrder: targets });
    assert.match(body, /\[#7\]\(https:\/\/github.com\/vizier-lab\/webui\/pull\/7\)/);
    assert.match(body, /bumped by the cascade/);
    assert.match(body, /Linked by: .*Submodule-PRs.*branch name/);
    assert.match(body, /`\/cascade-merge`/);
  });

  it('explains when there is nothing to link', () => {
    assert.match(
      renderLinksComment({ targets: [], gitlinks: [], warnings: [], cascadeOrder: [] }),
      /changes no submodule gitlinks/,
    );
    assert.match(
      renderLinksComment({ targets: [], gitlinks, warnings: [], cascadeOrder: [] }),
      /no matching PR was found/,
    );
  });

  it('surfaces warnings', () => {
    const body = renderLinksComment({ targets, gitlinks, warnings: ['something is off'], cascadeOrder: [] });
    assert.match(body, /> \[!WARNING\]\n> something is off/);
  });

  it('marks a linked PR it cannot read', () => {
    const body = renderLinksComment({
      targets: [{ repo: 'vizier-lab/design', number: 99, sources: ['marker'], missing: true }],
      gitlinks: [],
      warnings: [],
      cascadeOrder: [],
    });
    assert.match(body, /\*\*unreadable\*\*/);
    assert.match(body, /\| #99 \|/);
  });

  it('says so when everything is already merged', () => {
    const body = renderLinksComment({ targets, gitlinks, warnings: [], cascadeOrder: [] });
    assert.match(body, /Every linked submodule PR is already merged/);
    assert.doesNotMatch(body, /`\/cascade-merge`/);
  });
});
