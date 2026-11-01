# Code of Conduct

Vizier is a self-hosted AI agent framework, MIT-licensed and built in the open. It is also a small
project: one maintainer, a handful of contributors, and a growing number of people running agents of
their own. That only works if the place stays decent to be in.

This document says what decent means here, and what happens when it isn't. It applies to everyone
who takes part — contributors, maintainers, and the agents and bots that act on their behalf.

## Our pledge

We want participation in Vizier to be a harassment-free experience for everyone, regardless of age,
body size, visible or invisible disability, ethnicity, sex characteristics, gender identity and
expression, level of experience, education, socio-economic status, nationality, personal appearance,
race, caste, colour, religion, or sexual identity and orientation.

We pledge to act and interact in ways that contribute to an open, welcoming, diverse, inclusive and
healthy community. In practice that means: **being patient with people who are still learning, and
being precise with code that needs to change.**

## Our standards

Things that build this place:

- **Assume the person is trying.** "I've never written Rust" is a fine starting point here. The
  `good first issue` label exists for exactly that person — see
  [CONTRIBUTING.md](CONTRIBUTING.md#good-first-issue-is-reserved-for-people).
- **Critique the change, not the changer.** "This copies logic that already exists in
  `vector_memory`; it should live in one place" is review. "Did you even read the code?" is not.
- **Explain the why.** A link to the constitution, the spec, or the line that already does it beats a
  verdict. Where you can, suggest the next step.
- **Be honest about what you didn't verify.** "Untested on Windows" is useful; a confident claim
  nobody checked is not.
- **Respect that people self-host.** Their deployment, their data, their configuration. Don't ask
  someone to paste API keys, tokens, private prompts, memory bundles or logs containing any of them
  into an issue, and redact your own.
- **Respect the project's scope.** "No, that doesn't fit Vizier" is a legitimate answer, given with a
  reason. Being told no is not mistreatment.
- **Give credit.** Other people's ideas, code and reports are theirs; say so.

Things that don't belong here:

- Harassment, intimidation, stalking, or following someone across threads and repositories after
  they've asked you to stop.
- Insults, demeaning comments, personal or political attacks, and sustained disruption of a
  discussion.
- Sexualised language or imagery, and sexual attention or advances of any kind.
- Discriminatory jokes or slurs — about any of the characteristics in the pledge above.
- Publishing someone's private information, such as a physical or email address, without their
  explicit permission.
- Dismissing or mocking someone for what they don't know, or for the tools they use to learn.
- Using the project's spaces to scrape, spam or advertise to its users.
- Deliberately wasting reviewers' time: bad-faith reports, argument for its own sake, mass automated
  pull requests, or an issue thread used as a place to think out loud at length.

## Agents, bots and other automation

This is an agent framework, so this comes up more than it would elsewhere.

- **An agent's messages are its operator's responsibility.** What your bot posts, opens, labels or
  argues is on you, exactly as if you had typed it.
- **Automation does not get a different standard, and it does not get a louder voice.** A bot that
  floods issues, re-litigates a decision in a thread, or claims work reserved for people is handled
  the same way a person doing it would be — and the account behind it may be blocked from the
  project's spaces.
- **`good first issue` is for human learners.** Bots and automated agents must not claim, comment on
  or open pull requests against those issues.
- **Disclose AI assistance** in a pull request, in one line. The person who opens it is the author and
  answers the review.

## Scope

This code applies in every space the project runs: this repository and its issues, pull requests,
discussions and reviews, and the same in its submodule repositories (`design`, `webui`, `landing`,
`dummyplug`). It also applies when you are representing the project elsewhere — speaking about it at
an event, posting as a maintainer, or acting on its behalf in another community.

It does not govern your own Vizier deployment, or a conversation you have with an agent you host. That
is your house, and your rules.

## Enforcement

The maintainer, **Ganesha Danu Enastika** ([@blinfoldking](https://github.com/blinfoldking)), is
responsible for clarifying and enforcing these standards, and will take whatever action they judge
appropriate in response to anything they see that they consider inappropriate, threatening,
offensive or harmful — including editing or removing comments, commits, code, issues and other
contributions, and warning or banning an account.

Some honesty about capacity: this is a one-person project. Reports are read, but a response may take
a few days. If a report is urgent, say so in the subject.

**Conflicts of interest.** If the maintainer is the subject of a report, say so plainly and, where
possible, raise it with a regular contributor whose judgement you trust instead; the maintainer will
step back from handling that report.

## Reporting

If you experience or witness behaviour that breaks this code, report it to
**hi@blinfoldking.dev**, or by direct message to the maintainer on GitHub. Reports are handled
privately.

Please include what happened, where, when, and links or screenshots if you have them. You do not have
to explain why it bothered you, and you do not have to be sure it "counts" — say what you saw and
the maintainer will work it out.

**No retaliation.** Asking for help, reporting a problem, or being a witness to one must not cost
anyone anything. Deliberately retaliating against someone for a report is itself a violation, and
will be treated as one.

## Enforcement guidelines

These are the steps the maintainer will usually follow, in order of how serious the problem is. They
are guidelines, not a ladder that must be climbed: a serious first incident can skip straight to a
ban, and a good-faith misunderstanding can end at a quiet word.

**1. Correction** — for a small, clearly unintended slip: a sharp comment, a dismissive reply, a
mistake in tone.

> A private note from the maintainer explaining what was wrong and why it mattered, and asking for
> it not to happen again. A public apology may be asked for. No record is kept.

**2. Warning** — for a single incident or a run of them: repeated dismissiveness, a personal attack,
a slur, or ignoring a correction already given.

> A private warning with the behaviour spelled out and what is expected instead. No interaction with
> the people involved for a set period, including unprompted contact in issues and reviews. Breaking
> the terms leads to a temporary ban.

**3. Temporary ban** — for a serious violation, or a pattern that continues after a warning.

> A temporary ban from any interaction with the project — public and private, including comments on
> issues and pull requests, and any contact with the people involved. The ban's length reflects the
> severity. Breaking it leads to a permanent ban.

**4. Permanent ban** — for a pattern of violations, sustained inappropriate behaviour, harassment of
an individual, or aggression toward or disparagement of groups of people.

> A permanent ban from all interaction with the project, and from any space the project runs.

## Attribution

This code of conduct is adapted from the
[Contributor Covenant](https://www.contributor-covenant.org), version 2.1, available at
<https://www.contributor-covenant.org/version/2/1/code_of_conduct.html>.

The enforcement guidelines follow the Contributor Covenant's
[enforcement ladder](https://github.com/contributor-covenant/code-of-conduct/blob/main/enforcement-ladder.md).

The Contributor Covenant is licensed
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/), and this adaptation is shared under the
same terms.
