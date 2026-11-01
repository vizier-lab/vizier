# Asking the person

An agent sometimes needs something from a person rather than from a tool: a decision, a detail, or
a credential. Two built-in tools cover that, and they differ in one thing only — where the answer
goes.

| Tool | Ends the turn? | Where it works | What the answer is |
|---|---|---|---|
| `ask_question` | **yes** | WebUI, project chats, Discord, Telegram | the person's next message, as structured data |
| `ask_sensitive_input` | no | WebUI and project chats only | handed to the script that asked, and nowhere else |

## `ask_question`

Ask up to ten typed questions at once — free text, choose one, choose several, or yes/no — and
they are answered in one submission, in the order you gave them.

The call **ends your turn**: the questions *are* your answer. Whatever you wrote in the same step
is kept and shown with them. The person's reply arrives as your next turn, naming the set and each
question id with the value chosen, the label the person read, and `skipped` for anything optional
they left blank.

- Ask **last** in a script. A script that fails *after* asking withdraws the questions, so the
  person never sees a set the agent was told failed.
- One set per turn. A refused set does not use that up.
- A refusal names every problem at once, so one round is enough to fix the call.
- A pending set never expires, and a later set does not invalidate it.

On Discord the questions are buttons and a select with the platform's own text entry; on Telegram
an inline keyboard per question, with toggles for choose-several. A set too large for a platform's
controls falls back to a reply form, which still answers the whole set in one submission.

## `ask_sensitive_input`

Ask for one sensitive value — a token, a password — and receive it **in the script only**.

- The agent never sees it. It is not written to the conversation, the history, or the copy of the
  turn replayed to the model.
- The value is armed with the turn's redaction, so where the run's report, the log or the telemetry
  would have shown it they show `[redacted]`.
- The call does **not** end your turn: the turn keeps running while the person decides.
- The person can reject the dialog, dismiss it (Escape, a press outside, its close control), or
  never answer. Each gives a result you can branch on: `provided`, `declined` or `timeout`.
- Values are 8 to 65,536 bytes. A shorter value comes back with `protected: false`, because
  redaction cannot arm it — and the dialog says so before it is submitted.

### What the guarantee does not cover

Redaction is **exact-string** replacement. It does not follow a value you transform (base64, a
hash, a split or joined copy), and it does not protect anything you write yourself: a file, memory,
a scratch file, an artifact, an outbound request body. Pass the value straight to the tool that
needs it.

A person can also type the value into an ordinary message instead of the dialog. Nothing prevents
that, and such a message is stored like any other, so always ask through the tool and never in
prose. `ask_sensitive_input` also does not store anything: to keep a credential, pass it to
`set_secret`.

### Where it is not available

Only the WebUI and project chats can show a masked input. Discord and Telegram store what they
carry and expose it to their own administrators, so the tool is absent there and the agent asks in
prose as it did before. It is absent in every unattended turn too — a scheduled run, a dream, a
background piece, an inter-agent turn or a side chat.

## What is kept

A question set and its answers are part of the conversation: deleted with it, shown to whoever can
read it, and retained like any other message. An ask for a sensitive value records **that** one was
requested and what came of it — never the value.
