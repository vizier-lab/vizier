# Feature Specification: Live Voice Conversation

**Feature Branch**: `055-live-voice`

**Created**: 2026-11-08

**Status**: Draft (clarified 2026-11-08)

**Resumed from**: the draft of this same file, written on this branch earlier on 2026-11-08. The delta log at the end of this file lists every change made after resuming it.

**Input**: User description (muruyung, 2026-11-08): "STT and TTS in this project still have to be switched on by hand, and they cannot do live voice — a person and an agent talking directly. I need live voice between the agent and the person, with interruption in two ways: (1) it is interrupted when the person speaks, (2) there is a dedicated interrupt control. The feature is separate from the existing STT and TTS, so it has its own control for turning live voice on. Please produce the plan, diagrams and the other reports, for example additional packages that have to be added."

## Context

Speech in Vizier today works one piece at a time. A person records a clip and one message is transcribed from it; an agent asks for a waveform and receives the whole buffer at the end; a reply is spoken after the fact, when the message is already finished. Every one of those is switched on by hand per agent, and every one is a single round trip. Nobody can speak while the answer is still being produced, and nothing listens between turns.

A conversation does not work like that. A person starts talking without pressing anything, hears the answer while it is still forming, and cuts in the moment the answer goes wrong. That last part is what makes the difference: an answer takes seconds to arrive and much longer to be spoken, so somebody who cannot interrupt has to sit through a wrong answer to find out that it was wrong.

This specification adds that as its own capability: a Live Voice control in a conversation which holds a conversation open with one agent — listening, answering aloud, and stopping the moment the person speaks. It is switched on by the person, per conversation, and is off otherwise. The manual speech tools **leave the surface** in this release and are **left in the code** (C12): live voice takes their place on screen, and nothing about them is deleted.

The two interruptions are both first-class, because they cover different rooms. Speaking over the agent is what a person does naturally where they can be heard. The control is what works in a loud room, on a poor microphone, or for someone who cannot raise their voice over the agent at all.

## Clarifications

### Session 2026-11-08 (resumed draft, three open decisions closed)

The three open questions this specification carried are decided below. Each
names who decided it: a product call belongs to the User; a technical one this specification may
make on its own is marked *in planning*.

| # | Was | Decision | Decided by |
|C1|FR-035 — which surfaces|**The chat interface only**: the WebUI in a browser and the desktop app. Discord and Telegram voice are out of this release, because a bot joining a voice channel is a different audio path, a different permission model and a much larger scope.|in planning, consistent with the User's "separate from the existing STT and TTS" instruction|
|C2|FR-036 — provider strategy|**Cascade, and only cascade, in this release**: voice activity detection on the client, then the agent's existing recogniser on one utterance, then the ordinary agent turn, then the agent's existing voice synthesised in speakable segments. A speech-to-speech realtime pipeline is *not* built now; the seams leave room for one later.|in planning, forced by the code (constraint note below)|
|C3|FR-027 — what an interruption stops|**Both, in this order**: the audio stops at once, and the running turn is cancelled the way the existing stop control cancels it. Text already produced stays in the conversation, marked interrupted. A tool call already finished is not undone.|in planning: a correction that queues behind an answer nobody wants is not an interruption|
|C4|(not asked before) transport|**The existing per-agent WebSocket, JSON frames, base64 PCM16 in 100 ms chunks.** No WebRTC, no second socket, no new endpoint: WebRTC would add signalling, a TURN server and a dependency for a stream that runs between one browser and one server.|in planning. **Reopened as Q11 (2026-11-09 16:07, C13); not overturned.** This row is the implemented candidate and the measurement a media transport has to beat: what it costs, and what it cannot do (packet loss is a hole — no jitter buffer, no PLC, no FEC), are the figures in `plan.md`.|
|C5|(not asked before) where echo is cancelled|**In the browser**, through the capture constraints the recording path already asks the browser for, plus a guard while the reply is audible. No acoustic-echo-cancellation library is added.|in planning|

### Constraint discovered in the code (this changes what FR-013 can promise)

`crates/vizier-core/src/llm/sse.rs:1-3` states that the project reads every provider body whole -
*"no streaming API"*. A turn's frames carry a finished message
(`VizierResponseContent::Message { content, stats }`,
`crates/vizier-core/src/schema/response.rs:129-132`), not token deltas.

So "the reply is spoken while it is still being written" cannot mean token by token. What it can
mean, and what FR-013 now requires: **the reply is spoken segment by segment, starting at the first
speakable segment while the later ones are still being synthesised.** For a single long message the
first words are heard once that message's text exists, not while it is being generated. Token-level
speech needs a streaming turn protocol and is a separate feature.

### Session 2026-11-08 21:19 (the User changed the surface, the trigger and the scope)

Three changes from the User, all binding. One **reduces** scope, one **moves** a cost; neither
loosens a requirement already agreed.

| # | Change | What it replaces | Consequence recorded, not hidden |
|---|---|---|---|
| **C6** | **The session has a screen of its own, in the manner of a call** — not a bar inside the conversation | the bar-in-composer surface (its amendment, \`20d97316\`, stays in the history and is marked archived below) | new FR-043 to FR-047 and User Story 6; the entry point stays a control in the conversation. **Superseded in part by C9 (2026-11-09): the surface floats over the conversation instead of replacing it, and there is no route.** |
| **C7** | **The interrupt control goes to the backlog.** The person interrupts by speaking; there is no button in this release | FR-020, FR-021 and User Story 3, which stay written and are marked deferred | the feature is *less* controllable on purpose: a noisy room, a bad microphone or a quiet voice has no escape hatch until that backlog item lands |
| **C8** | **Barge-in waits for the person to finish speaking.** The start of speech no longer interrupts; the end of the utterance (the ~800 ms hangover firing) does | FR-019's "at once, before their words have been transcribed", and the ~250 ms onset guard | while the person speaks the agent **keeps talking** — overlapping audio is now the intended behaviour, and SC-002's clock moves to the end of the utterance |

**The consequence the plan has to answer (C8), stated plainly.** The session decides the person has
finished by hearing ~800 ms of quiet on the microphone. Under C8 the agent is *still speaking*
during that window, so the microphone also hears the agent. On speakers without headphones that
quiet may never arrive and the interruption may never fire. FR-047 therefore makes "the end of the
person's speech is detected while the reply is audible" a requirement rather than a hope, SC-008
measures it, and \`plan.md\` holds the fallback (ducking the reply's volume once the person is
detected speaking). None of that changes C8 itself.

### Session 2026-11-09 06:52 (the surface floats, and only one session exists)

Two changes from the User. Neither is a preference about pixels: the first removes a navigation
problem, the second removes an entire class of "which conversation is talking?" bugs.

| # | Change | What it replaces | Consequence recorded, not hidden |
|---|---|---|---|
| **C9** | **The call floats.** It is a panel above the conversation that the person can drag, in the manner of a desktop call window — not a full screen, and not a route of its own | C6's *"in place of the conversation view"*, the call-screen layout, and the designer's recommended `/live` route | FR-043 to FR-046 rewritten, **FR-048** and **FR-049** new, US6 rewritten, wireframe replaced. The chat stays behind the panel, readable and scrollable, which is what C6's full screen could not promise. **Superseded by C11 (2026-11-09 16:07): the call is one row in the conversation; FR-048 and Q9 retire with the panel.** |
| **C10** | **One live session in total.** Starting one in another conversation — or with another agent — does not fail and does not open a second: the call **moves** there and the old session closes | FR-005's per-conversation rule, the edge case that allowed two sessions on two conversations, and open question Q4 | FR-005 rewritten, **FR-050**, **FR-051** and **FR-052** new, **SC-011** new. This is the one place where the feature gets *less* permissive on purpose |
| **C11** | **The call is one row inside the conversation** — not a floating panel, not a full screen, not a route | C9's draggable panel, C6's call screen and the composer bar before it (all three archived, none deleted) | **FR-043, FR-044, FR-045, FR-046 and FR-049** rewritten around a row; **FR-048** and **Q9** superseded, because a row has no position to clamp or remember; US6 rewritten. Surviving from the panel work: the state words, the rule that the microphone indicator is never the element that gives way, and the `ended`/`failed` treatment |
| **C12** | **The manual speech-to-text and text-to-speech surfaces are deprecated — the surfaces, not the code.** The record control and the spoken-reply toggle leave the conversation's UI; the providers, tools, configuration and build capability stay, because another surface may use them later | the assumption that both surfaces must be offered side by side in the composer | **FR-053** new. **FR-003 and FR-041 are NOT superseded and get stronger**: two paths live side by side while only one is on screen, so a build without live voice must still leave every existing speech feature working. The "flag off" tests (T014, T015) stay alive for the same reason |
| **C13** | **Audio travels in a lane of its own on the conversation's transport** — droppable under backpressure, while everything a person reads keeps the ordered broadcast. *Which* transport carried it was **Q11**, opened here and **closed on the socket** at 16:35 | R1 (audio as base64 PCM16 JSON on the topic socket) *as the final answer* | **FR-054, FR-055, FR-056, FR-057, FR-060** new. The per-connection lane, `LIVE_VOICE_SOCKET_BACKLOG = 16` and `stall` are the shipped path, not a candidate. The latency criteria are measured on the path that carries the sound. Choosing the transport was the Lead's call on measured numbers, and **Q11** records both the question and the answer |
| **C14** | **The audio capability is standalone**: a documented contract a third party can implement without our client | the assumption that the audio path is a private detail of the WebUI | **FR-058** and **FR-059** new, **SC-012** new, and a public-contract section in the frame contract. It also binds our own WebUI to the same contract, or the contract rots. **Narrowed by C15**: a document is not enough — access needs a scoped API key |
| **C15** | **Third-party access is by API key, scoped to live voice** — the User's reading of "standalone": not only a document, but a feature another system can actually call, kept safe by a key | C14's "the conversation's own credential", which would have handed an outside system a person's whole account | **FR-061 to FR-064** new, **SC-013** new, and the contract's Authentication section rewritten. The gap is named, not glossed: API keys exist but carry no scope (`middleware.rs:200` grants the owner's full permissions), so this is code, not prose |
| **C16** | **The reply arrives as it is made, not when it is finished** — and starting it *before* the person stops talking is a separate, larger question (Q12) | the constraint note of 2026-11-08, FR-012's "not character by character", and FR-013's exclusion of anything finer than a segment | **FR-065 to FR-070** new and **PROVISIONAL** (not in this release's gate), **SC-014** new, **Q12** opened with the User's answer being "wait". The User chose this direction on 2026-11-10 06:39 and deferred speculation; the numbers that would size it are still the Lead's. FR-012/FR-013 are amended in place, and **FR-023's premise is broken and must be restated** — its justification ("already stored before its first word was heard") is true only while replies arrive whole |

**Why C9 is not just a repaint of C6.** C6 put the call in place of the conversation, and then had to
answer what happens to the chat — the full-screen render answered it by growing a transcript panel
beside the call, which is the chat again, in a second layout that must be kept in step forever. A
floating panel does not replace anything: the conversation is where it was, and the call is a window
over it. One surface instead of two, and no navigation to design.

**Why C10 is a real simplification.** Before it, every question about the session — who owns it, what a
second tab sees, what happens to the microphone when two people start one — had a per-conversation
answer that the person had to keep in their head. With one session, the rule is one sentence: *the call
is wherever you last started it*, and starting it somewhere else moves it.

### Session 2026-11-09 16:07 (a row instead of a panel, the old tools retire, the audio becomes a real call)

Four instructions from the owner of the project, relayed by the User. Three change what gets built;
the fourth turns part of it into a **public contract**, which is the larger commitment of the four.

**One row, and the panel is over.** The User chose the single-row surface after seeing both candidates
drawn. The reasoning that decided it is worth keeping: a floating panel has to be positioned, clamped,
remembered and dismissed, and every one of those is a way to lose the call off the edge of a window —
while the row's worst failure is that it takes one line of height. C11 replaces C9, and the consequences
are honest: FR-048 (drag) and Q9 (remembered position) **retire**, because a row has no position.

**The old speech surfaces retire; their code does not.** The record control and the spoken-reply toggle
leave the conversation's UI, because live voice is where a person speaks now. What the User asked for
explicitly is that the **Rust side stay intact** — providers, tools, configuration and build capability
are kept for another surface later, so this is a retirement of *what is offered*, not of *what exists*.
That distinction changes two requirements the other way round from the obvious reading: **FR-003 and
FR-041 are not weakened, they matter more.** Two speech paths now live side by side while only one is on
screen, so "a build without live voice leaves every existing speech feature working" (FR-041) and "the
old tools' switches neither start nor block live voice" (FR-003) are exactly what keeps the overlap
honest. The "flag off" tests stay alive for the same reason.

**Audio gets a lane of its own; the transport itself stays the one we already have.** The User's
instruction was WebRTC for the sound with the transcript left on the existing WebSocket, and immediately
afterwards the same instruction came with an escape clause worth honouring: if a cheaper way gets the
same result, say so first. That clause is what closed **Q11** on the socket (16:35) — so what this
document fixes is the **shape**, and the shape survived the decision: the sound travels in a lane of its
own (FR-054), everything a person reads stays on the conversation's ordered path (FR-055), and the audio
is bound to exactly one session (FR-056). The JSON path already built — 100 ms base64 chunks, the
per-connection lane, the socket backlog, the `stall` frame — is therefore **the shipped path**, and the
alternative's measured cost stays in `plan.md` rather than in the requirements. What did **not** change
either way: the transcript, the state word and everything a person reads stay on the conversation's
socket, because a second copy of the transcript travelling with the audio is how two histories start
disagreeing.

**Two consequences that are easy to miss, and are requirements now.** Audio in its own lane has a
lifetime nothing like a message's: it can begin before the session exists, outlive the session that
opened it, or arrive for a conversation that has since had a *different* call. FR-056 therefore binds it
to exactly one session and refuses it otherwise, rather than letting it join whatever is live now. And
because the sound is droppable while text is not, SC-001 and SC-002 are measured **on the lane that
carries it** — a number taken from the message hop would describe a path
that is not carrying the sound.

### Session 2026-11-09 16:35 (the socket carries the sound, and a stranger gets in with a key)

Two decisions from the User, and the second one changed a requirement rather than adding one.

**Q11 is closed: the socket.** *"kalo liat concern yang disampein SA sama lead sih gw setuju pake
websocket dulu aja kalo gitu."* The carrier is `ws-json-pcm16` — the path that already exists, with 46
green client tests, a per-connection lane, a bounded queue and a `stall` report. Three things this
settles, and one it does not:

- **Settled**: no setup frames for a peer, no peer registry, no Opus decoder on the server, no bundled
  relay. Those tasks are not implemented and must not be revived from an old branch without the numbers
  that lost here being re-read.
- **Settled**: the reliability the socket cannot give — jitter buffer, packet-loss concealment, FEC — is
  **accepted as missing**. Packet loss is a hole in the sound, not a retry. Saying so now is cheaper than
  discovering it in a bad-network report later.
- **Still open, in another place**: echo in a full-volume speaker and talk-over are handled by T047's
  ducking and the 150 ms guard, and whether that is enough is **SC-008's measurement**, not a promise
  made here.
- **Kept**: the measured cost of the option that lost (str0m +22 crates, 36 s, 583 MB, 1,25 GB peak RSS,
  Opus decoder, relay) stays in `plan.md`. A future decision should meet those numbers, not rediscover
  them.

**"Standalone" now means an outside system can actually call it — and that needs a key.** The first
reading of the User's request was a *document*: a contract someone could implement. The correction is
narrower and sharper: *"iya si fitur live voice ini bisa diintegrasiin di luar client vizier, dan dia
(client lain) butuh API key buat akses live voice ini biar aman."* A document plus **an API key scoped to
live voice**. That is FR-061 to FR-064, and it contradicts what this spec said twelve minutes earlier
(auth = the conversation's credential, no second token). The correction is recorded rather than quietly
overwritten, because the reason matters: handing a third-party system a conversation credential hands it
a person's account, and the entire point of the key is that it does **not**.

**The uncomfortable part, said plainly.** API keys already exist (`storage/user.rs:69-81`) and already
authenticate (`auth/middleware.rs:183-209`), but they carry **no scope** — line 200 resolves a key to the
owner's full role and permissions. So today, giving a partner an API key to use live voice would give
them everything the person can do. FR-062 is therefore not a documentation task: it needs a capability on
the key and a check at every route, and until it exists, third-party access MUST NOT be offered. That is
the one place in this delta where a security requirement is written against code that is not there yet,
and it is written that way on purpose.

### Session 2026-11-09 17:31 (which row, and what a token count actually is)

**The surface is chosen: the composer's own meta line.** The User picked **B1-1** — *"lebih enak liatnya"* —
from the variants drawn for them. So the call is not a rail above the composer (that was B2, drawn and not
chosen) and not a row in the thread (B3): the line that today carries the context ring and the model chip
becomes the call's row, at **0 px** added height. FR-043 is reworded to say exactly that, and **B1-2 is
retired** — it lost, and it is recorded here so nobody rebuilds it from a sheet that is still on disk.

**The User then asked a question that changed the requirement.** *"itu tuh token count apaan ya bro?
Bukannya existing tuh count token tuh ada di dalem setiap chat ya bukan di dalem text box?"* They were
right, and the sheet was wrong. What the sheets drew as a **124 px block of `222k in · 4.7k out`** is the
per-message stats line under an agent's reply (`webui/app/lib/messages.ts:88`, rendered by
`MessageList.tsx:167`). The composer's meta line does **not** carry that: it carries a **context ring,
18×18 px** (`design/src/components/ContextRing/ContextRing.tsx:26,37`, mounted at
`design/src/components/Composer/Composer.tsx:238`) whose label is `222k of 200k tokens`, and the model
chip at **223,3 px** (`webui/app/screens/chat/ModelOverridePicker.tsx:19,94`). Text token counts appear in
the composer only on mobile (`webui/app/screens/chat/Composer.tsx:411-415`).

**What that means for the requirement, stated rather than smoothed over.** During a session the row gives
up the **context ring (18 px) and the model chip (223,3 px)** — not a metrics block that never existed.
The row therefore has **106 px more free space** than the sheets assumed, so the sheet's `waveform 379 px`
is **not a contract value** and MUST be recomputed from real geometry when the row is built. Two
consequences are recorded instead of being quietly dropped: the sacrificed set is smaller than drawn, and
one number in the handoff is void.

**The surrender order is now one order.** Four different orders were in play — FR-049's vague "its own
detail first", T038's `level → elapsed → agent name`, the sheet's `chip → waveform`, and the Lead's
`nowrap` ruling — and T094 has to test "in the stated order", which cannot be written against four
answers. The Lead settled it (2026-11-09, measured from the source, not from the sheet): **model chip**
shrinks to 96 px then goes (cut to its last segment), **waveform** 39 → 12 bars → gone, **context ring**
goes whole; and **never** shrink: the microphone indicator, the state word, the elapsed time, the end
action, the idle control — all `nowrap`, with no ellipsis ever in the state word. Floor **292 px** — the row's own arithmetic, not a round number (FR-049) — and
the row never wraps to a second line. FR-049 carries the order, T038 implements it, T094 tests it.

### Session 2026-11-10 06:35 (the reply no longer has to arrive whole)

The person used the feature and said what the numbers had not: *"berarti possible kalo gw pengen dibikin
jadi streaming LLM nya? Jadi ga perlu kalimat utuh, cuma dia tetep nahan buat kasih respon selama user
masih berbicara tapi dia sendiri sembari streaming udah ada konteks untuk balasannya."* Two ideas are in
that sentence, and they are not the same size. Separating them is the whole point of this entry.

**First idea: the reply is delivered as it is made.** Today a turn delivers a finished message
(`VizierResponseContent::Message`, `crates/vizier-core/src/schema/response.rs:129-132`), and the
project's LLM client reads every provider body whole (`crates/vizier-core/src/llm/sse.rs:1-3`). So the
first word cannot be heard until the last word exists. That is the constraint this document recorded on
2026-11-08 (constraint note, `spec.md:38-49`), and it is why FR-013 promises segment-by-segment
*synthesis* rather than segment-by-segment *arrival*. The person is asking for the second thing:
segments that **reach them** while later ones are still being written. That needs a turn protocol that
can carry a reply in pieces, and it is what FR-065 to FR-068 write down.

**Second idea: the reply is started before the person stops talking.** That needs partial recognition —
the recogniser seen through the conversation rather than as one clip — because the cascade recognises a
whole utterance at a time (`stt/mod.rs`, `transcribe(&[u8], …)`). It also needs a rule for what to do
when the person changes direction mid-sentence: a partial answer built on a half-heard question is
either thrown away (paying for tokens nobody reads) or kept (answering the question they abandoned). That
is **Q12**, with "not in this release" as its default, because it is a different feature from streaming
the reply out, and bundling them would hide the larger one behind the smaller.

**What this does to requirements already written.** Three of them assumed a whole message:

- **FR-012** (`spec.md:447`) says the reply appears "as soon as the turn produces its message, not
  character by character". That stays true of characters and stops being true of segments: with FR-065 a
  reply can appear in pieces. The sentence is amended, not withdrawn.
- **FR-013** (`spec.md:448`) excludes token-by-token speech *because* the turn delivers a finished
  message. FR-065 removes that reason for segments; per-token speech remains out, because speaking
  punctuation fragments is worse listening, not better.
- **FR-023** (`spec.md:463`) is the sharp one, and it is easy to miss: it justifies keeping the whole
  interrupted reply by saying the message "was already stored before its first word was heard". With
  segments arriving as they are made, that premise is gone — an interruption can now land when only the
  first third exists. The requirement must say what is kept when what was produced is all there is, and
  that is why FR-066 and FR-067 exist.
- **FR-027** (`spec.md:467`) and **FR-022** (`spec.md:462`) already forbid speaking a cancelled reply
  afterwards; they now have to cover segments **in flight** and not only segments queued.
- **T076**'s marks (`tasks.md:281`) survive unchanged. Its clock starts at the end of the utterance and
  stops at the first sample that is actually audible; a streamed reply makes that stop earlier, and does
  not redefine it. What it cannot measure is the tail — how long until the reply *finishes* — and that is
  why SC-014 is a shape criterion rather than a new latency number.
- **RK-1** (`review.md:239, 254`, the Lead's file) recorded the risk that live voice is only as live as
  the model's whole-response latency, with no default taken. FR-065 is the answer to that risk, which
  makes this a **revision of a User decision**, not a contradiction to be resolved later. The line
  itself stays the Lead's to update.

**The direction is decided; the numbers are not.** On 2026-11-10 06:39 the User chose the first idea and
deferred the second: *"level 1 dulu aja deh bro, tapi kalo pas gw tes terus latency nya masih kerasa
ganggu baru gw minta level 2"*. So FR-065 to FR-070 are the agreed direction, not a proposal — and they
stay **PROVISIONAL** for a different reason: no measurement says how much they buy. The Lead's assessment
gives the sizes (11.1 s today; ~2.5–4 s once FR-017 is delivered correctly; a further ~1–2 s from
streaming), and until those numbers are re-measured against a real turn, a latency target would be a
number invented to look finished. **Angka menyusul dari pengukuran** — recorded here as absent on
purpose, not omitted.

**Two requirements exist because the obvious version of this change breaks things.** FR-069 keeps the
finished message arriving at the end: the segments are an addition on the way to the same end state, so a
client that knows nothing about them — including yesterday's third-party client and our own history —
keeps working. FR-070 makes the added frames a *contract change with its compatibility stated*, because
this contract is public (FR-058) and a client must not discover a new frame by breaking on it.

**Status: provisional, and honestly so.** These six requirements are written now so the behaviour is
testable the moment it is built — they are **not part of this release's gate**, and no latency number is
attached to them until the assessment's figures are re-measured. Writing them as prose-only would have
repeated the mistake this feature has already paid for twice: a promise in a sentence, no carrier,
nothing to test.

### What "standalone" means here, and what it costs

The User asked for the media part to be able to stand alone so that someone else could implement their
own client against it, and asked what that actually means. In one paragraph — and note this holds
because it is a requirement (FR-058) rather than a property of any transport — and with Q11 closed on the
socket it is now the *only* way an outside client gets in (FR-061 to FR-064):

> **Standalone means the audio capability is a thing a stranger can use without our client.** A person
> who wants live voice inside *their own* application does not fork this repository, does not import our
> React components, and does not read our state machine. They read one published document: where to
> connect, how to identify the conversation and the call, how to authenticate, what audio to send, what
> they will receive, how a call starts and ends and moves, and what the failures are called. Then they
> write their own client against it, and it works.

**The minimum that has to be public** (each one becomes a requirement in FR-058, and the wire shapes
belong in `contracts/`):

| Surface | Why it cannot be private |
|---|---|
| **How a call is set up** | whatever the audio path needs before sound can move — in WebRTC that is an offer, an answer and candidates; on a socket it is an open and a first frame. A third party must be able to do it on a transport they already have |
| **Session identity** | they must be able to say *this audio belongs to that conversation's call*, and be refused when it does not (FR-056) |
| **Authentication** | an **API key scoped to live voice**, and nothing wider (FR-061, FR-062). Not the conversation's credential: a key that carries a person's whole account is not a key, it is a password |
| **The audio shape** | sample rate, channels, frame duration, and the acceptable codec or format: the thing that makes a non-browser client possible at all (FR-057) |
| **Lifecycle** | start, move, end, and the reconnect window, expressed as steps rather than as one client's behaviour |
| **Failure vocabulary** | the same reason words our own UI shows, so a third-party client can show something truthful |

**What stays internal, deliberately** (FR-059): the segmenter, the generation counter, the session
registry, the provider credentials, and the shape of our own client state. Those are ours to change;
the six surfaces above are not, once this ships.

**The cost, stated plainly.** A public contract is a promise with a support burden attached: every
message in the setup sequence and every reason word becomes something we cannot rename quietly. That is why
FR-058 also binds **our own WebUI to the same contract** — the one client that is always in the
building, so a contract we break shows up as our own breakage first. If the reviewer prefers not to
carry that burden, the honest alternative is to say so and drop C14 explicitly, not to publish a
contract and then drift from it.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — A conversation without typing (Priority: P1)

A person opens a conversation, presses Live Voice, and asks a question out loud. The answer is spoken in segments as soon as the turn's message exists (FR-012, FR-013: this turn protocol delivers a finished message, not token deltas). They ask a follow-up without touching anything, then press the control again to stop; the whole exchange is in the conversation as ordinary messages, ready to be read, searched and continued by typing.

**Why this priority**: This is the request itself. On its own it already replaces typing for a person who would rather talk, and every other story only makes that loop safer or more controllable.

**Independent Test**: In a conversation whose agent has a working recognizer and voice, press the control, ask a short question, hear the answer, ask a follow-up with no further press, and end the session. Both questions and both answers are in the conversation as text.

**Acceptance Scenarios**:

1. **Given** a conversation with Live Voice off, **When** the person presses the control and speaks a question, **Then** the question appears as their message, the reply appears as the agent's message, and the reply is heard.
2. **Given** a spoken question, **When** the person stays silent for a few seconds, **Then** the session is still listening and the microphone is still open.
3. **Given** a live session, **When** the person asks a follow-up without pressing anything, **Then** it is submitted and answered like the first question.
4. **Given** a live session, **When** the person presses the control again, **Then** the session ends, the microphone is released, and every turn remains in the conversation.
5. **Given** a finished session, **When** the conversation is opened later, **Then** the voice turns read exactly like typed ones and typing continues the conversation normally.
6. **Given** Live Voice has never been used, **When** the person uses the conversation as before, **Then** nothing about it changes — the same messages, the same remaining controls, and the speech tools' own settings untouched (FR-040).

---

### User Story 2 — Stopping the agent by speaking (Priority: P1)

The agent is halfway through an answer that has gone the wrong way. The person says "no, the other file" out loud. The agent keeps talking for as long as they are talking — and the moment they stop, it stops. Their correction is now the next message, and the answer that follows picks it up. They never reach for the interface.

**Why this priority**: Talking over someone is the natural way to stop them. Without it, live voice is an announcement that cannot be argued with.

**Independent Test**: Have the agent speak a long answer, speak over it, and confirm that (a) the agent is still audible while the person speaks, (b) the audio stops once the end-of-utterance period passes, before the transcript arrives, (c) those words arrive as the person's message, and (d) the interrupted answer is marked as interrupted.

**Acceptance Scenarios**:

1. **Given** the agent is speaking, **When** the person begins to speak, **Then** the agent keeps speaking (C8) and the session is already capturing their words — no silence is expected yet.
2. **Given** the person has finished speaking, **When** the end-of-utterance period passes (default 800 ms), **Then** the audio stops and the running turn is cancelled (FR-019, FR-027), before the transcript has arrived.
3. **Given** that interruption, **When** transcription finishes, **Then** the words appear as the person's message and are answered, and the interrupted reply stays in the conversation marked as interrupted.
4. **Given** an interrupted reply, **When** the next reply is spoken, **Then** no part of the interrupted one is heard again.
5. **Given** the device's own speakers and no headphones, **When** the agent speaks at high volume, **Then** the session does not treat its own voice as the person, and the end of the person's speech is still detected while the reply is audible (FR-026, FR-047).
6. **Given** the person makes a sound too brief to be a word, **When** the end-of-utterance period passes, **Then** the reply is neither cancelled nor resumed-in-two-pieces, no message is sent, and the session is listening again (FR-024).
7. **Given** the person speaks and the reply happens to finish on its own first, **When** the end-of-utterance period passes, **Then** nothing is cancelled, and their words simply become the next message.

---

### User Story 3 — Stopping the agent with a control (Priority: backlog — deferred 2026-11-08)

> **Deferred to the backlog (User decision, 2026-11-08 21:19, C7).** This release has **no interrupt
> control**: the person interrupts by speaking (FR-019). The story is kept in full so the requirement
> is not lost, and it is **not** part of the gate for this feature — no acceptance scenario below is
> required for this release to be finished, and its task block in `tasks.md` is marked `[BACKLOG]`.
> The stated cost, accepted by the User: where speaking over the agent does not work — a noisy room,
> a poor microphone, a quiet voice — there is no way to stop the reply in this release except ending
> the session.

In a noisy café, on a bad microphone, or with a quiet voice, the person cannot speak over the agent. They press the interrupt control beside the Live Voice control. The voice stops at once, the session stays live, and they can type the correction, or say it once the room is quieter.

**Why this priority**: A person who cannot out-talk the agent would otherwise have no way to stop a wrong answer without ending the whole session — which makes the feature unusable exactly where it is most wanted.

**Independent Test**: With the agent speaking, press interrupt; the audio stops and the session still listens. Then type a message and confirm it is answered **as text** while the session stays live and silent (FR-018 withdrawn, 2026-11-08).

**Acceptance Scenarios**:

1. **Given** the agent is speaking, **When** the person presses interrupt, **Then** the audio stops at once and the session stays live.
2. **Given** an interrupted session, **When** the person types a message, **Then** it is answered as text, the session stays live, and nothing is spoken for that reply.
3. **Given** the agent has not started speaking yet, **When** the person presses interrupt, **Then** the session stays live and needs no restart; what happens to the running turn follows FR-027.
4. **Given** an interrupted but live session, **When** no further speech arrives for the idle period, **Then** the session ends like any other.

---

### User Story 4 — A session never keeps the microphone (Priority: P2)

A person closes the tab in the middle of an answer, or walks away leaving the session live. The voice stops, the microphone indicator goes out, and the conversation keeps the text. When they come back the control is off, and nothing listens until they press it.

**Why this priority**: A live microphone is the risk this feature introduces. Everything else is worth nothing if the microphone stays open when the person believes it is closed.

**Independent Test**: With a live session, close the page; confirm the operating system's microphone indicator goes out and the reply's text is saved. Leave a session silent past the idle period; confirm it ends and says why.

**Acceptance Scenarios**:

1. **Given** a live session, **When** the person leaves the conversation or closes the page, **Then** the audio stops, the microphone is released, and a reply in flight finishes as text without being spoken.
2. **Given** a live session and no speech, **When** the idle period passes, **Then** the session ends with a visible reason and the microphone is released.
3. **Given** a live session, **When** microphone permission is withdrawn in the browser or the operating system, **Then** the session ends at once and does not try the microphone again.
4. **Given** a live session, **When** the agent is deleted or restarted, or the conversation is deleted, **Then** the session ends and an answer in flight is not spoken.
5. **Given** any session that ended, **When** the person looks at the conversation, **Then** its text is intact and the control reads off.
6. **Given** a live session in one tab, **When** another tab or another person opens the same conversation, **Then** they are told a session is active and cannot start a second one.

---

### User Story 5 — A broken session explains itself (Priority: P2)

Recognition comes back empty, the voice provider errors in the middle of an answer, or the network drops. The session says what happened, keeps the text, and either recovers or ends with a reason. It never sits there looking live while doing nothing.

**Why this priority**: Every provider in this path is a network call that can fail. Silence that looks like listening is the failure mode people trust least.

**Independent Test**: Cut the connection mid-answer; the session shows reconnecting and stops the audio. Restore it within the limit and the same conversation continues. Cut it for longer and the session ends with a reason, with the answer's text still in the conversation.

**Acceptance Scenarios**:

1. **Given** a live session, **When** recognition returns nothing intelligible, **Then** the person is told, no message is sent, and the session listens again.
2. **Given** an answer being spoken, **When** the voice provider fails, **Then** the rest of the answer still appears as text, the failure is reported, and the session listens again.
3. **Given** a live session, **When** the connection drops, **Then** the state reads reconnecting within 2 seconds and the audio stops; **When** it returns within 30 seconds, **Then** the same session continues without duplicating the message in flight.
4. **Given** the connection does not return in that time, **When** the wait ends, **Then** the session ends with the reason and the text is kept.

---

### User Story 6 — The call is one row in the conversation (Priority: P1)

A person presses Live Voice and a single row appears in the conversation they are already in: the
agent's name, what it is doing, an open microphone, and one control that ends the call. Nothing
floats over the transcript, nothing has to be dragged out of the way, and no message is ever covered.

**Why this priority**: The User chose the row over the floating panel. It is the cheapest surface that
still answers the two questions a person actually asks — *is it hearing me?* and *is it thinking or
gone?* — which is what FR-006, FR-038 and FR-042 exist for.

**Independent Test**: From a conversation, press Live Voice; the composer's meta line becomes the row
carrying the state word, the microphone indicator and one End control; ask something and watch it read
`thinking`; end the call; the row is gone, both turns present, the control off, and no message was
covered at any window width.

**Acceptance Scenarios**:

1. **Given** a conversation with Live Voice off, **When** the person presses the control, **Then** the composer's meta line becomes the call's row — no line added, nothing above the composer moving — and the session starts (FR-043).
2. **Given** the live row, **When** the person looks at it, **Then** it shows the agent, the state word, the thinking indicator when the agent is working, and that the microphone is open — all without a reload (FR-006, FR-042, FR-038).
3. **Given** the live row, **When** the agent is silent for a long time because its answer is slow, **Then** the row says the agent is thinking and does not read as a disconnected call (FR-042).
4. **Given** the live row in a narrow window, **When** the row runs out of room, **Then** it gives up its own detail in FR-049's order — the model chip first and gone, then the agent's name shortened, then the waveform — and keeps the microphone indicator, the state word, the elapsed time and the End action whole, with the agent's name and the thinking indicator still present, down to its 292 px floor — and it never covers a message (FR-049, FR-038, FR-042).
5. **Given** the live row, **When** the person ends the call, **Then** the microphone is released, the row disappears, and the conversation keeps every turn (FR-028, FR-034, FR-045).
6. **Given** the live row, **When** the person closes the tab or the window, **Then** the session ends and nothing keeps listening (FR-028).
7. **Given** a call ended by any means, **When** the conversation is opened again, **Then** it is unchanged and the entry control is off (FR-046).
8. **Given** a live call in one conversation, **When** the person starts one in another conversation or with another agent, **Then** the call there starts, the old one closes by itself, and the old conversation says why it closed (FR-050, FR-051).

---

### Interaction Flow (interruption by speech, C8; the control is deferred, C7)

```mermaid
flowchart TD
    off["Live Voice off"] -->|"press the control"| row["Composer meta line becomes the live row, microphone open"]
    row --> listen["Listening"]
    listen -->|"person speaks"| transc["Speech becomes the person's message"]
    transc --> turn["Agent turn runs, reply text appears"]
    turn --> speak["Reply spoken in segments once its message exists"]
    speak -->|"reply ends"| listen
    speak -->|"person speaks over it, the agent keeps talking"| talkover["Their words are captured while the reply plays on"]
    talkover -->|"they finish: end-of-utterance period passes"| barge["Audio stops, the running turn is cancelled"]
    barge --> transc
    listen -->|"press end on the row"| ended["Session ended, microphone released, row gone"]
    speak -->|"press end on the row"| ended
    listen -->|"silence past the idle period"| ended
    speak -->|"page closed or permission withdrawn"| ended
```

Every path that leaves `listen` or `speak` except the interruption by speech ends the session, and every
ending releases the microphone (FR-028, FR-029). The interrupt control of the earlier drafts is not in
this diagram because it is deferred (C7).

### Edge Cases

- **The person speaks over the agent without headphones**: the device's speakers feed the microphone, and under C8 the agent is *still talking* while the person talks. Two things must hold at once: the session must not treat its own voice as the person at any output volume, and it must still hear the end of the person's speech over that voice (FR-026, FR-047). A session that needs silence to notice the person has finished is a broken session here, not a tuning problem.
- **A second person in the room, or a television**: the session cannot tell one voice from another — the detector is an energy gate on one microphone, with no speaker identity. So background speech above the gate may be transcribed and may interrupt, exactly as it would over any voice assistant; what the session must not do is treat ambient noise below the gate as speech, and what the person must be able to do is correct it by speaking again or by typing. **Real attribution (diarization) is not promised here and would be its own specification.** *(Corrected in the Lead's review, 2026-11-08: the earlier wording promised a capability the chosen detector does not have.)*
- **A very short utterance** ("yes", "no", "stop"): recognised and submitted as a message rather than discarded as noise. A sound too brief to be a word neither cancels the reply nor becomes a message (FR-024, Story 2 scenario 6) — the threshold is the length that makes a barge-in, not a character count.
- **The reply finishes while the person is still talking**: nothing is left to cancel; their words become the next message and the session returns to listening (Story 2 scenario 7).
- **The person talks over the agent for a long time**: the reply plays on underneath until the person stops, which is the cost C8 accepted; nothing queues up afterwards, because the reply is cancelled once, not paused.
- **A long monologue**: an utterance is cut at the maximum length (default 60 seconds) and submitted, so a person speaking for minutes still gets answers.
- **Silence in the middle of a sentence**: a pause shorter than the end-of-utterance period does not split the message.
- **The person speaks while the agent is still thinking**, before any audio: their words become the next message; whether the running turn is also cancelled is the decision in FR-027.
- **The person types while the agent is speaking**: the reply keeps playing and is not cancelled (FR-025 deferred with C7) — the typed turn simply queues and is answered as ordinary chat, in text only, because speech belongs to the session (FR-018 withdrawn). The audio stops when the person stops speaking, never on a keystroke.
- **An answer full of code, a table or a long URL**: the conversation keeps all of it; the spoken form skips what cannot be read aloud and says the rest is in the conversation.
- **The provider is slow**: waiting is visible, the session never appears to be listening while an answer is still pending, and a stalled provider ends in a reported failure rather than an endless wait. Since FR-017 lets the agent think longer, this is the common case rather than the exception, which is what FR-042's indicator is for.
- **The microphone is unplugged mid-session** or the device is switched: the session reports the lost device and does not silently stay "live".
- **The page is reloaded mid-session**: the session ends with the page, the microphone is released, and the conversation is intact; a reloaded page starts with the control off.
- **The person is on a phone browser and the screen locks**: the session must treat the resulting suspension as an end with a reason, not as a live session that stopped hearing.
- **A person speaking another language** than the conversation's configured one: the transcript may be wrong; the session is expected to follow the configured language, and changing it mid-session is out of scope.
- **A call started in a second conversation, or with another agent**: the call moves there and the first one closes by itself, with a reason left behind (FR-050, FR-051, FR-052). There is never a moment with two microphones open, and never a refusal that leaves the person with no call at all (C10).
- **A second tab on the same conversation**: it is the *same* conversation, so the question is whether a press there moves the call to that tab or is refused — open question Q2, default refused. This is the one case C10 does not answer by itself.
- **The row in a narrow window or a long conversation**: the row gives up its own detail in FR-049's order and no other — the model chip to its floor and then gone, the agent's name shortened only after that, the waveform 39 → 12 bars and gone, the context ring whole — and never gives up the microphone indicator (FR-038), the state word (FR-042), the elapsed time or the End action. It does not overlap a message, does not change the composer's height, and does not turn into two lines down to its 292 px floor: a row that grows is a panel again.
- **A dream, a scheduled task or a background report**: no live voice; the capability exists only while a person has a session open.
- **A conversation in a mode that restricts what the agent may do** (a Plan chat): a voice message follows the same rules as a typed one, including any refusal.
- **Vizier restarts mid-session**: the session ends; the conversation and every completed turn survive.
- **A build without the capability**: the control does not appear and every existing speech feature behaves as before.

## Requirements *(mandatory)*

### Functional Requirements

**The control and the session**

- **FR-001**: Every conversation MUST offer a dedicated Live Voice control, visibly separate from the attachment control and from whatever manual speech surfaces remain; pressing it opens the live row (FR-043). It MUST be off by default in every conversation and on every visit, and its state MUST NOT be remembered between visits — except that starting a call **moves** it (FR-050).
- **FR-002**: Pressing the control MUST start a live voice session on that conversation with its agent and show the live row. Ending it MUST be possible from the row for the whole session (FR-045). Where a call is already live elsewhere, pressing it MUST move the call here rather than refuse (FR-050); the control MUST show which of the two a press will do.
- **FR-003**: Live Voice MUST be independent of the existing speech-to-text and text-to-speech tools in both directions: starting it MUST NOT enable, disable or reconfigure those tools, and their switches MUST neither start it nor block it.
- **FR-004**: Starting MUST require both a usable recognizer and a usable voice for that agent. When one is missing, the control MUST name what is missing and how to set it, and MUST NOT start; a session that can only listen, or only speak, MUST NOT be presented as live voice.
- **FR-005**: A session MUST belong to one person, and there MUST be **at most one** live session for that person across every conversation and every agent (C10). Only someone allowed to chat in a conversation may start or end a call there. Starting a call **in another conversation or with another agent** MUST NOT fail with "a session is already active": it **moves** the call and says why in the conversation it leaves (FR-050, FR-051), so two are never running. The **one** refusal in this feature is narrower: a second connection pressing start on the **same** conversation that already has a call held by the owner's other connection (Q2's default). That is refused with `kind: "session_active"` — the same word the contract uses, never a different one — because moving would need a rule for audio already captured in the first connection, and the person can end the call from the connection that holds it.
- **FR-006**: The **live row** MUST show, without a page reload: connecting, listening, waiting for the agent, speaking, interrupted, reconnecting, ended with a reason, failed with a reason. (`off` is the absence of the row, not a state the row renders.) **The copy is English and stays English** (User, 2026-11-10 09:27): the eight words above are the shipped words, and translating them is a new decision rather than an edit — `agent sedang berpikir` measures 130 px against `thinking`'s 47 px, so a language change re-measures the row that FR-049 fixes at 24 px.
- **FR-007**: A voice message MUST run the same turn as a typed one: same conversation history, same mode rules, same tools, same permissions, same memory, same usage accounting. Live Voice MUST NOT add, remove or alter the agent's tools, memory or permissions.
- **FR-008**: A session MUST use the agent's own model and speech configuration. Live Voice MUST NOT silently change what typed turns use.

**The conversation loop**

- **FR-009**: The session MUST decide on its own when the person starts and stops speaking; the person MUST NOT press anything between turns.
- **FR-010**: When the person stops speaking for a short pause (default 800 ms), or an utterance reaches the maximum length (default 60 seconds), the utterance MUST be transcribed and submitted as a message in that conversation.
- **FR-011**: Each submitted message MUST appear in the conversation as that person's message, marked as spoken, with the transcript as its content. The session MUST NOT add anything else to the conversation on the person's behalf.
- **FR-012**: The agent's reply text MUST appear in the conversation and MUST be spoken aloud in the same session; the spoken words MUST correspond to the text shown. "Appear" means as soon as the turn produces that part of it, and never character by character. *(Reworded in the Lead's review, 2026-11-08: the original said "as it is generated", which this turn protocol cannot do. Amended 2026-11-10 by C16: while a turn delivers a finished message the whole reply appears at once, and if FR-065 is built the reply appears in segments — either way the unit is a segment, never a character.)*
- **FR-013**: Speaking a reply MUST begin at its first speakable segment, and each further segment MUST be synthesised while the previous one is being heard, so that a long answer is preceded by no long silence and does not wait for its own last sentence. (What is excluded is speech finer than a segment: a character or a half-clause is worse to listen to, not faster to understand. The 2026-11-08 reason for that exclusion — that a turn delivers a finished message — is about *arrival*, and C16's FR-065 removes it for segments while leaving token-by-token speech out.)
- **FR-014**: The spoken form MUST NOT read formatting marks aloud (punctuation used as markup, code fences, raw URLs). What cannot be spoken MUST be skipped with a short notice that the conversation holds the rest to read.
- **FR-015**: After a reply ends, or after an interruption, the session MUST return to listening without a press.
- **FR-016**: Raw audio MUST NOT be stored. Only the transcript and the reply text are kept, as for typed turns, under the same access rules and retention as the conversation.
- **FR-017**: With each voice message the agent MUST be told that the message came by speech and that its reply will be spoken, so it can answer in a form that suits listening: **short by default**, because the reply is heard rather than read. That indication MUST apply to that turn only and MUST NOT change what a typed turn sees. **The agent decides how long to take, not the person**: when the question genuinely needs reasoning or a lookup, the agent MAY take longer and answer at length, and there MUST be no user-facing switch, mode or setting that makes that choice. *(Rewritten in the Lead's review, 2026-11-08, on the User's decision of that day: default short answers, longer only when the question needs it, decided by the agent.)*
- ~~**FR-018**: Typing MUST stay available during a session: a message typed while a session is live is answered like any other, and its reply is spoken.~~ **WITHDRAWN on the User's decision, 2026-11-08 17:27: a typed message is answered as text only.** Speech belongs to the live session alone. Typing during a session still works, and its reply comes back as text only. It does **not** stop the audio either: FR-025 is deferred with the interrupt control (C7), so in this release the only way to stop the reply is to stop speaking (FR-019). Nothing in this feature links a typed turn to the session's voice.

- **FR-042**: While the agent is working and nothing is audible — the case FR-017's longer answers make long — the interface MUST show that the agent is thinking, and MUST show it as a different state from `reconnecting` and from `ended`. It MUST appear as soon as the turn is accepted (before the first audio, and before the model's own latency), and MUST clear when the first segment is heard, when the turn ends, or when the session ends. It is a state the session already has (`waiting for the agent`, FR-006); what this requires is that the person can tell "thinking" from "gone", on screen, without waiting for sound. *(Added on the User's decision, 2026-11-08 17:27.)*

**Interruption**

- **FR-019**: While the agent is speaking, the person's speech MUST NOT stop the audio by itself. When the person has **finished** speaking — the end-of-utterance period elapsing, default 800 ms (FR-010) — the audio MUST stop and the running turn MUST be cancelled (FR-027), before their words have been transcribed. Overlap while the person speaks is intended (C8), and the end of their speech MUST still be detected while the reply is audible (FR-047).
- ~~**FR-020**: A dedicated interrupt control MUST be available for the whole of a live session. Pressing it MUST stop the agent's audio at once and MUST NOT end the session.~~ **DEFERRED to the backlog on the User's decision, 2026-11-08 21:19 (C7).** This release has no interrupt control; FR-019 is the only way to interrupt. Kept here so the requirement is not lost, and it is not part of this release's gate.
- ~~**FR-021**: The interrupt control MUST work at any moment of a session; pressed while nothing is being spoken, it MUST leave the session live and unchanged.~~ **DEFERRED with FR-020 (C7).**
- **FR-022**: After an interruption, no part of the interrupted reply MUST be heard later, and the session MUST NOT resume speaking it on its own. The reply's remaining segments are abandoned, not paused: a later reply starts from its own first segment.
- **FR-023**: An interrupted reply MUST remain in the conversation as the agent's message, marked as interrupted, so the person can see what they did not hear. How much text there is depends on when the interruption lands: interrupting while the reply is being spoken keeps the whole message, which was already stored before its first word was heard — **and that reason holds only while a reply arrives whole** (C16): once segments arrive as they are made, an interruption can land with only part of the reply produced, and then what is kept is what was produced, marked interrupted. Interrupting while the model is still producing it ends the turn as an abort (`VizierResponseContent::Abort` is a turn that produced nothing, `crates/vizier-core/src/schema/response.rs:156-163`), so the person sees the interrupted mark and the turn's saved activity trail, not partial prose. *(Clarified in the Lead's review, 2026-11-08: "readable as far as it was produced" promised partial text the abort path does not keep.)*
- **FR-024**: The utterance that caused an interruption MUST become the person's next message, unless it was too short to be one, in which case nothing is sent **and the reply is not cancelled for it** — the threshold is the length that makes a barge-in, not a character count (Story 2 scenario 6).
- ~~**FR-025**: A message typed while the agent is speaking MUST stop the audio and cancel the running turn, on the same path as any other interruption.~~ **DEFERRED with FR-020 (C7), User's decision 2026-11-08 21:19.** This release has no interrupt by typing either: the only way to stop the audio is to stop speaking (FR-019) — not a button, not a keystroke. Typing during a session is ordinary chat (FR-040) and does not stop the audio. Its only carrier, T054, sits in the deferred US3, and the row carries exactly one action, End (FR-045), so a live requirement would have been a promise with no one to keep it. *(Deferred in the Lead's review, 2026-11-09: a requirement whose only task is in the backlog must not read as a release promise.)*
- **FR-026**: The session MUST NOT interrupt itself: its own audio coming back through the device's speakers MUST NOT be taken as the person speaking, at any output volume and with or without headphones.
- **FR-027**: An interruption MUST stop the audio at once and MUST also cancel the running turn, on the same path as the existing stop control: text produced so far stays in the conversation marked interrupted, nothing further is generated or spoken, and a tool call already completed is not undone. When the agent was working rather than speaking, an interruption MUST behave the same way, so "I interrupted while it was editing files" has exactly one answer. The trigger in this release is the end of the person's utterance (FR-019), never its start. *(Decided in Clarifications C3; trigger moved by C8.)*

**Ending, silence and failure**

- **FR-028**: A session MUST end when the person presses the control again, leaves the conversation or closes the page, microphone permission is withdrawn, the conversation is deleted, the agent is deleted, restarted or reconfigured, or the connection is lost beyond the reconnect limit. In every case the audio MUST stop and the microphone MUST be released.
- **FR-029**: A session MUST end after the person has been silent for the idle period (default 3 minutes), with a visible reason and the microphone released, so a forgotten session cannot listen on. **The owner of that ending is the server** (User, 2026-11-10 10:20, option A): it counts the silence, ends the session itself and sends `state: ended` with reason `idle`, and the client's part is only to release the microphone when that arrives. Three things follow from putting it there rather than in the browser: the clock is reset by **every** event that makes a session active — a start, an utterance closing, a segment of a reply going out — so a session that is being used never dies mid-answer; it **stops counting while the agent is working**, for the same reason; and a client that hangs, crashes or was written by somebody else cannot leave a microphone listening, because it is not the one holding the timer. The contract gained the matching lifecycle row — `end by itself` in `contracts/live-voice-frames.md` — because until 2026-11-10 `idle` was a word in the vocabulary with nothing that produced it.
- **FR-030**: When microphone access is refused or no device is available, the session MUST fail with a clear reason, MUST NOT be left half-started, and MUST be retryable without reloading the page.
- **FR-031**: When recognition yields nothing intelligible, the person MUST be told, no message MUST be sent, and the session MUST return to listening.
- **FR-032**: When the voice provider fails while speaking, the rest of the reply MUST still appear as text, the failure MUST be reported, and the session MUST return to listening.
- **FR-033**: A lost connection MUST show as reconnecting and stop the audio within 2 seconds. If the connection returns within 30 seconds the same session MUST continue without duplicating the message in flight; otherwise the session MUST end with the reason.
- **FR-034**: Ending a session for any reason MUST NOT delete or alter the conversation's text. A reply in flight when the session ends MUST finish as text and MUST NOT be spoken.

**Speech capabilities**

- **FR-035**: Live Voice MUST be offered on the chat interface only — the WebUI in a browser and the desktop app — and MUST NOT be offered on the Discord or Telegram channels in this release. A channel that cannot carry it MUST behave exactly as it does today. *(Decided in Clarifications C1.)*
- **FR-036**: The session MUST obtain recognition and speech in a cascade — the utterance recognised as one clip, the reply produced by the ordinary turn, the reply synthesised in segments — through the recogniser and the voice the agent is already configured with. It MUST NOT require a speech-to-speech or realtime provider, and MUST NOT add a per-minute cost for a session that carries no more audio than its own turns. *(Decided in Clarifications C2.)*
- **FR-036a**: The session and its speech MUST sit behind one seam, so a realtime speech-to-speech pipeline can be added later behind configuration without changing the frames, the controls or this feature's outcomes.
- **FR-037**: Recognition MUST follow the conversation's configured speech language. Where the agent already lets a person choose that language, a session MUST be able to use the chosen one.
- **FR-038**: For the whole of a session the interface MUST show that the microphone is open, including when the window is not focused.
- **FR-039**: Documentation MUST cover starting and ending a session, what audio is sent to which provider, what is stored, the two ways to interrupt, and the limits (idle stop, one session per conversation).

**Unchanged behaviour**

- **FR-040**: With Live Voice never used, every existing behaviour MUST be unchanged: the speech tools, automatic transcription of uploaded audio, requested spoken replies, text chat, and the HTTP and WebSocket surface as existing clients see it.
- **FR-041**: Live Voice MUST NOT be required by any existing capability; a build without it MUST leave every existing speech and chat feature working.

**The row in the conversation (C11)**

- **FR-043**: Starting a session MUST turn the composer's **own meta line** into the call's row — the line that today carries the context ring and the model chip — at **0 px** added height (C11, surface B1-1). It MUST NOT float over the conversation, MUST NOT be a movable panel, MUST NOT be a separate rail above the composer, MUST NOT be a route of its own, and MUST NOT cover a message; the entry point MUST stay a control in the composer (FR-001). When the session ends the line goes back to what it was, with nothing left behind.
- **FR-044**: The row MUST show, for the whole session: the agent the call is with, the state word of FR-006, the thinking indicator of FR-042 when the agent is working, that the microphone is open (FR-038), and the one action of FR-045. These are the row's minimum; what else it carries is open question Q6.
- **FR-045**: The row MUST offer a way to end the call for the whole session, reachable without leaving the conversation, which stops the audio, releases the microphone and ends the session (FR-028). With the interrupt control deferred (C7), ending the call is the only action the row offers — and the row MUST NOT present a second control whose only effect is the same thing.
- **FR-046**: Showing and ending the row MUST NOT change the conversation: the same messages, the same text, nothing rewritten, reordered or re-timed (FR-034).
- **FR-047**: The session MUST detect the end of the person's speech **while the agent's reply is audible** (C8): the reply continuing MUST NOT prevent the end-of-utterance period from elapsing. Echo cancellation alone MAY be the mechanism; where it is not enough, the reply's audibility to the microphone (not to the person) MUST be reduced once the person is detected speaking, so that the interruption still fires. The measured form of this is SC-008.
- ~~**FR-048**: The panel MUST be draggable by its header with a pointer, MUST keep its whole area inside the window at every position (no edge can be dragged out of reach), MUST remain usable when the window is resized smaller than where it was left, and MUST keep its position while the call lives. Whether the position survives the call is open question Q9. Its size MUST NOT change when it is moved or when the conversation behind it is scrolled.~~ **SUPERSEDED by C11, 2026-11-09 16:07: the call is a row.** A row has no position to clamp, no edge to drag out of reach and nothing to resize, so this requirement and Q9 retire together. Kept because the panel remains the fallback if the row is rejected.
- **FR-049**: The row MUST cost **no extra height**: it takes over the composer's existing meta line — **24 px, 0 px added** to a 156 px composer — so the conversation and the input do not move when a session starts or ends (C11, surface B1-1). It MUST NOT cover, clip or reorder any message: the conversation scrolls to its own end exactly as it does with no session, and the composer stays usable at every width. When space runs short the row gives up its own detail **in this order and only this order**: (1) the **model chip** shrinks and then goes, and only after it the **agent's name**: the chip to **96 px**, cut to its last segment, because the provider prefix is not the informative half, and then gone entirely; the agent's name shortens only once the chip is gone, truncated with its tail ellipsised, and is the last of the two to go. The chip is given up first because the agent's name is required (FR-044) while the chip is Q6's optional extra — holding the optional element while dropping the required one would invert the row's own priorities; (2) the **waveform** shortens from 39 bars to 12 and then goes, never below 6 px between bars; (3) the **context ring** goes whole, never partially drawn. **The microphone indicator (FR-038), the state word (FR-042), the elapsed time, the end action (FR-045) and the idle control MUST NOT shrink, truncate or wrap at any width**: they are `nowrap`, and no ellipsis may ever appear in the state word. The row's floor is **292 px**, and it is measured rather than chosen: the never-shrink set is **276 px** with the thinking marker at its measured 20 px — state word 70 + marker 20 + microphone indicator 58 + elapsed time 34 + end action 54 = 236, plus the five 8 px gaps that the six flex children carry in every state = 40 (`LiveVoiceRow.tsx:207-276`, `ROW` at `:32`; `paddingInline` is on the idle pill `:180` and the terminal button `:263`, never on the live row `:211`, so it does not inflate this) — and 292 leaves 16 px for the middle slot, which holds the transcript, the reason or the waveform. Two consequences must not be re-derived away: the marker's `<span>` is in the DOM in every state (`:224`, an empty string when the state is neither `waiting` nor `reconnecting`), so the gaps stay five and the fixed minimum **without** a visible marker is **256 px**, not 248 and not 240; and because FR-044 makes the thinking indicator part of the row's minimum, the floor has to cover the marker's case rather than the cheaper one. The floor was **240 px** until 2026-11-10, which is **36 px below the row's own fixed content**: at that width the row cannot be rendered at all, with or without the marker, so the old floor guaranteed a broken row instead of a narrow one. At the minimum window (900 px) the pane leaves it far above 292, so the floor guards a pane narrowed by surrounding chrome; if it is ever crossed the row keeps the never-shrink set and drops everything else rather than wrapping, because a row that grows to two lines is a panel again. In the chosen surface the context ring is already given up for the session — the state word takes its slot — so the path in practice begins at the chip; the three steps are stated in full so the test has one answer and a surface that keeps the ring does not need a new rule.

**Why the agent's name is in the first step and not in the never-shrink set (2026-11-10).** FR-044 requires the row to show the agent the call is with, and until today the row did not — it was implied by the header, which is a different surface (the same reading FR-038 already rejects for the microphone: a tooltip is not "show"). Adding it to a 24 px line has one cost that cannot be argued away: **width is finite**, and a long agent name would eat the transcript, which is the only element on that line carrying words. So it surrenders first, in the same step as the model chip, and it truncates rather than wraps. On this surface the state word already holds the ring's slot, so in practice the first thing to go is still the chip; the name's inclusion here means a build that keeps the ring, or an agent with an unusually long name, still has one stated answer instead of an invented one.
- **FR-050**: Starting a call while one is live **elsewhere** — another conversation, or another agent — MUST move the call there rather than fail: the row appears on the new conversation, the old session ends by itself, and the old row disappears (C10). The move MUST be one event, not two sessions that briefly overlap; the microphone MUST end up open once, on the new call.
- **FR-051**: The conversation a call is taken *from* MUST say why it stopped — a visible reason, recorded where the session's other endings are, so a person who comes back to it is not left wondering whether it is still listening (FR-029, FR-030). Whether that reason is a message in the conversation or only a notice is open question Q10.
- **FR-052**: A move MUST NOT strand a turn: if the agent was answering when the call was taken, that turn MUST be cancelled and its partial text kept, exactly as any other interruption (FR-027); it MUST NOT keep generating into a conversation nobody is listening to, and its reply MUST NOT be spoken into the new call.

**The manual speech controls leave the UI, the code stays (C12)**

- **FR-053**: The manual speech-to-text and text-to-speech **surfaces** MUST be removed from the conversation's UI in favour of live voice — the record control and the spoken-reply toggle are no longer offered. Their **implementation MUST NOT be removed**: providers, tools, configuration and build capability stay exactly as they are, because another surface may use them later. During the overlap live voice MUST work whether or not those tools are enabled (FR-003), and a build without live voice MUST keep every one of them working (FR-041).

**The audio path (C13 — transport under evaluation)**

- **FR-054**: The person's audio and the agent's audio MUST travel on the conversation's **own WebSocket**, in a lane of its own on that socket, with delivery rules of its own: audio frames go to the one connection that owns the session and are **droppable** under backpressure, while every frame a person reads keeps the ordered broadcast (`review.md` D9). **The consequence, stated rather than glossed:** sharing the socket means a burst of speech cannot evict text from the session ring and a busy socket cannot lose the call — but it also means **a stalled socket silences the sound along with the text**, and **a lost packet is a hole in the sound**, because this path has no jitter buffer, no packet-loss concealment and no FEC. Q11 chose that trade on measured cost (2026-11-09 16:35); the alternative's price is recorded in `plan.md`. It MUST be two-way and realtime enough that a person can interrupt the agent by speaking (FR-019), and a reply heard through speakers MUST NOT re-enter the microphone as a new utterance (FR-046, FR-047). **Echo cancellation is required of the feature, not of the transport**: it comes from the capture constraints on `getUserMedia` (C5, already in `webui/app/lib/liveVoice/capture.ts:34`), so it MUST NOT be counted as an argument for any transport. The carrier is `ws-json-pcm16` — base64 PCM16 in 100 ms chunks. `media-track` stays a **reserved** contract value, not a shipped one; no peer setup, no server-side audio decoder and no relay are part of this feature.
- **FR-060**: Live voice MUST NOT require the person to obtain a third-party service or an account: if it needs a relay, the relay MUST ship with Vizier and be configured by us, not by the person (User, 2026-11-09 16:16), and its configuration MUST be documented well enough that a third-party client can reach it (FR-058). **This is satisfied vacuously in this release and that is recorded on purpose**: Q11 chose the socket (2026-11-09 16:35), the socket needs no relay, so no relay ships and there is nothing to configure. Kept rather than deleted, because the moment anything relays audio the promise applies again, and a reader who finds it missing would re-open the question the User already answered.

**Third-party access — the API key (C15, 2026-11-09 16:35)**

- **FR-061**: Live voice MUST be usable by a client that is not Vizier's own, authenticated by an **API key** issued for that purpose — not by a person's WebUI session and not by an agent's provider credential. The key MUST identify the person whose session it starts, so FR-005's one-call-per-person rule still holds across both kinds of client.
- **FR-062**: An API key issued for live voice MUST grant **live voice and nothing else**. It MUST NOT reach another agent's history, another conversation, settings, memory, files, jobs or any other endpoint, and it MUST NOT read or widen the owner's role or permissions. A key that cannot be narrowed MUST NOT be offered for third-party use at all — an unscoped key handed to an outside system is one person's whole account.
- **FR-063**: The key MUST have a lifecycle that a person can act on: it is created with a name, may expire, can be rotated, and can be revoked. **Revoking a key — and rotating it, which is a new key plus the old one retired — MUST take effect on a running session that key opened**: within 1 s the session ends with `key_revoked` from the contract's vocabulary, the microphone is released (FR-028) and the turn in flight is cancelled (FR-052's path). It is honoured on the connection that is already open, not deferred to the next connection. **Expiry is FR-063a**: the same three consequences, a different moment, and no time bound.
- **FR-063a**: **Expiry** MUST be enforced on the connection that is already open, and the moment is stated rather than implied: the lapse is noticed at the **next frame the client sends** on that connection, not by a timer (Lead's decision, 2026-11-10 — a sweep timer for a rare event is not built in this release; `specs/_antrean.md`, `GI-03`). When it fires, the consequences are FR-063's: `ended` with `key_expired`, the microphone released (FR-028), the turn in flight cancelled (FR-052's path). **What this release does not promise, and what a client must not infer from FR-063:** no bound on the time between the lapse and the ending. A client that sends nothing keeps its session past the key's expiry, and a turn already running keeps running until a frame arrives or the connection closes — ending it on the second would need exactly the timer this release declines. `SC-013`'s one second is about **revocation**; it is not a promise about expiry and must not be quoted for it. A new connection is refused at the door whatever the old one is doing (FR-064).
- **FR-064**: A request whose key is missing, revoked, expired or scoped away from live voice MUST be **refused before any session state is created**, with a reason a client can show. The refusal MUST NOT reveal whether a session, a conversation or an agent exists to a caller who is not allowed to use it.

**The reply as it is made (C16, 2026-11-10 — provisional, not in this release's gate)**

- **FR-065**: The reply MUST NOT be required to arrive whole. Its text and its speech MUST reach the person **as segments**, the first leaving the server as soon as it exists, and the client MUST begin playing a segment as soon as that segment arrives rather than after the last one does. A reply that is still being produced MUST NOT hold the words already produced. *(PROVISIONAL: recorded so it is testable when the Lead's assessment says it is worth building; not part of this release's gate — see Q12 and C16.)*
- **FR-066**: An interruption MUST stop the **remainder**, not only the queue: when the person interrupts while segments are still arriving, no segment that had not already reached them may be heard afterwards, and the reply stops where it stopped. This extends FR-022 and FR-027 to a reply still in flight — an interruption is a stop, never a pause.
- **FR-067**: What is stored MUST be the reply that was made, once: no text duplicated by re-delivery, no truncated tail because the last segment arrived after the session ended, and the same text after a reload as the person heard. What they read later MUST NOT disagree with what they heard.
- **FR-068**: If a session ends or fails while segments are still arriving, the row MUST say something that stays true afterwards: the reason, and that the reply stops where it stopped. It MUST NOT present a partial reply as complete, and MUST NOT present a reply that was heard as failed.
- **FR-069**: A reply that arrives in pieces MUST also arrive **whole**, once: after the last segment, the turn MUST still deliver the finished message it delivers today, and the conversation MUST store exactly one copy of it. A client that knows nothing about segments MUST keep working — it receives the whole message and ignores the segments, and history, transcription and reload are unchanged. *(Derived from C16: the pieces are an addition on the way to the same end state, not a replacement for it.)*
- **FR-070**: The segment frames MUST be published as a **change to the contract**, with its compatibility stated rather than assumed. The contract carries no revision number today, so this change MUST say which kind it is — an additive frame a client may ignore, or a break a client must handle — and MUST give a client a way to tell which revision it is speaking to. A third-party client that met the contract yesterday MUST NOT discover the change by breaking. *(FR-058 makes this contract public; that is what makes an unannounced frame a defect rather than an implementation detail.)*

*(Code reality behind FR-062, verified 2026-11-09: API keys exist — `crates/vizier-core/src/storage/user.rs:69-81` — but carry **no scope**: `crates/vizier-http/src/auth/middleware.rs:200` resolves a key to the owner's full role and permissions. So the scoping this requirement demands is **not built**, and calling it a documentation change would be wrong.)*
- **FR-055**: Everything that is not audio MUST stay on the conversation's existing transport — the state word, the transcript, the reply's text, errors and endings. What a person reads in the conversation MUST be the transcript the conversation stores, never a parallel copy carried by the audio path.
- **FR-056**: The audio path MUST be correlated with exactly one session (FR-005) in exactly one conversation. Audio belonging to a session that has ended, or arriving for a conversation where no session is live, MUST be refused rather than adopted by whatever session is live there now.
- **FR-057**: The audio the server accepts MUST be described in terms another client can meet — sample rate, channel count, frame duration, and what codec or format is acceptable — rather than being whatever one client happens to send. A client that meets the description MUST be accepted, whichever browser or library produced it.

**The public contract (C14)**

- **FR-058**: The capability MUST be usable by a client that is **not this WebUI**. Everything such a client needs — how to reach us, what identifies the session, how it authenticates, what audio to send and in what shape, how a call starts, moves and ends, and what the failures are called — MUST be written down as a public contract. **This WebUI MUST itself use that contract**, with no private shortcut, or the contract decays into documentation nobody executes.
- **FR-059**: What a third-party client MUST NOT need: this repository's client code, its state machine, the segmenter, the generation counter, the session registry, or any provider credential. Those stay internal, and their absence from the contract is deliberate rather than an oversight.

### Key Entities

- **Live voice session**: one person, one conversation, one agent. Has a state, a start time, an end reason, and the recognizer and voice it used. Exists only while live.
- **Voice message**: a conversation message that came from speech or was spoken. Carries whether it was voice-originated, whether it was interrupted, and how much of it was spoken. Read, searched and deleted like any other message.
- **Utterance**: one continuous stretch of the person's speech inside a session, from the pause before it to the pause after it. Produces at most one message, or none when it is noise.
- **Interruption**: one event, caused by the person finishing an utterance (FR-019). The interrupt control (FR-020) and the typed message (FR-025) would be two more causes and are both deferred with C7. Records when it happened and how far into the reply, and whether the turn continued (FR-027).
- **Live voice setting**: per agent — which recognizer and which voice a session uses. Separate from the speech tools' own enablement.

## Success Criteria *(mandatory)*

### Measurable Outcomes

**How these are verified, and with what** (User, 2026-11-10 09:27): the QA environment is the Ubuntu machine that runs this branch, and it verifies **without any database access** — no `mysql-local`, no direct reads of a live store. Fixtures are the only data source. A criterion that can only be shown against real stored data is therefore recorded as **"cannot be checked"** rather than replaced by an assumption, and that label is a result in its own right: it says the criterion is unproved, not that it passed.

*Every latency figure below is measured on the **path that carries the sound** — the conversation's socket, `ws-json-pcm16` — with the marks taken in the client, not on the message hop. A number taken from a path that is not carrying audio describes nothing.*

- **SC-001**: From the person finishing a short question to the first audible word of the answer, on an ordinary laptop and ordinary broadband, with a hosted recogniser and voice: **p90 <= 3.5 s and p99 <= 5 s is the figure this document records, and it is not a release gate** (User, 2026-11-09 19:16: there is no baseline from a real provider yet, so a gate would pass or fail by luck — the recorder in `latency.md` measures and does not judge); 2.0 s at p50 is the stretch target. The reason for the figure, not the aspiration: the plan's budget floors at 2.57 s before the model has produced anything (800 ms end-of-utterance + 120 ms to the wire + 900 ms hosted recognition + 100 ms turn start + 600 ms first segment + 50 ms to sound), and the turn protocol delivers the model's whole message rather than token deltas, so the model's own latency sits on top of that floor. A local-only recogniser or voice is CPU-bound, is expected to be slower still, and is measured separately (open question Q3). *(Restated in the Lead's review, 2026-11-08: the earlier 2 s p90 could not be met by the architecture the plan itself describes.)*
- **SC-002**: From the **end** of the person's speech (the end-of-utterance period firing) to silence: within 300 ms in at least 95% of attempts, and the next answer addresses the interrupting message in at least 90% of those attempts. The agent being audible *during* the person's speech is not a failure — it is the behaviour C8 asks for, so the clock starts when they stop, not when they start, and it is measured on the client (first audible sample of the reply to silence).
- ~~**SC-003**: The interrupt control silences the agent within 200 ms in every attempt, in 100 of 100 tries.~~ **Not measured in this release: the control is deferred (C7).** Kept so the criterion returns with the backlog item.
- **SC-004**: In a 10-minute session of at least 15 turns, at least 90% of turns need no interaction other than speaking, and every spoken message appears in the conversation — no lost turns in 100%.
- **SC-005**: A person who has never used the feature is talking to the agent within 20 seconds of first seeing the control, permission prompt included, on the first attempt, in at least 90% of tries and without reading documentation.
- **SC-006**: With Live Voice never used, every existing speech behaviour is unchanged, verified by the existing checks for those features.
- **SC-007**: A session left with no speech ends within its idle period in 100% of cases, and the operating system's microphone indicator is off within 2 seconds of any ending.
- **SC-008**: With the device's speakers at 80% volume and no headphones, a 10-minute conversation produces no self-interruption — and, with the reply audible throughout, the end of the person's speech is detected within the end-of-utterance period in 95 of 100 attempts. A session that only notices the person once the reply happens to end fails this criterion.
- **SC-009**: A failed call never leaves the session looking live: in every scenario of FR-030 to FR-033 the person sees a reason within 5 seconds and the microphone is released or the session is ended.
- **SC-010**: Pressing the control shows an interactive row within 500 ms on the first attempt, on a fresh profile, with microphone permission granted at the system prompt — in at least 90% of tries. The clock stops at the row, not at the first answer (SC-005 keeps the wider "talking within 20 seconds").
- **SC-011**: A call taken by another conversation leaves no second session behind: in 100 of 100 tries exactly one microphone is open after the move, the old conversation carries a reason, and no turn keeps generating or is spoken into the new call (FR-050, FR-051, FR-052).
- **SC-012**: A client written against the published contract alone — no code from this repository — completes a full session (start, speak, hear the reply, end) in 100 of 100 attempts, on a machine that has never held this repository. This WebUI is one such client: same contract, same test.
- **SC-013**: A key issued for live voice starts a session and speaks through it in 100 of 100 tries, and is refused by **every** endpoint outside live voice — checked against an explicit list of the other routes, not a sample — in 100 of 100 tries. A key that is revoked mid-session ends that session within 1 s with its reason, in 100 of 100 tries.
- **SC-014**: For a reply long enough to be more than one segment, the first segment is audible **before the last segment is produced** — in 100 of 100 tries with a hosted provider, with both marks taken on the client (first sample audible, last segment received). This is a shape, not a speed: it says the reply arrives while it is being made, and it holds even on a slow model. *(Belongs to FR-065, and therefore to Q12's answer rather than to this release's gate.)*

## Assumptions

- **Surface**: settled — the chat interface only, the WebUI in a browser and the desktop app (FR-035, Clarifications C1).
- **Provider strategy**: settled — the cascade in FR-036 (Clarifications C2), with the pipeline interchangeable behind one seam.
- **Interruption semantics**: settled — the audio stops and the turn is cancelled (FR-027, Clarifications C3).
- **Defaults chosen here** (800 ms end-of-utterance, 60-second longest utterance, 3-minute idle stop, 30-second reconnect, **one session in total** across conversations and agents, off by default): all are defaults, may be tuned during planning, and do not change the intent.
- **Nothing is stored beyond text**: audio is never written to disk, so a person can read a session but not re-listen to it. Keeping audio would be a separate decision with its own retention and access rules.
- **The device is the one running the interface**: microphone and speakers are on the person's machine, which is also where the desktop app runs. Headphones improve the experience; echo cancellation is expected to make them optional, and a session that interrupts itself is a hard failure (SC-008).
- **Speech quality is the provider's**: recognition mistakes are not corrected by this feature; a person fixes a bad transcript by speaking again or by typing. Editing a sent message is out of scope.
- **Language**: one language per conversation, the agent's configured one. Switching language mid-session, and per-utterance detection, are out of scope.
- **No new agent capability**: voice is another way to send a message, not a new tool. Nothing becomes available to the agent that typing does not already allow, and the mode of the conversation still decides what it may do.
- **Non-goals**: wake words and always-listening sessions; a session with no conversation open; multi-person or conference capture; telephone calls; recording or re-listening to audio; speech on the Discord and Telegram channels beyond the decision in FR-035; on-device-only operation as a guarantee.
- **Cost**: the person's provider account pays. A session costs roughly what its turns cost plus its speech, so the idle stop is the main guard, and FR-036 is where a per-minute provider would change that picture.
- **Optional capability**: per the constitution the feature sits behind an optional build capability, added without changing the shape of the existing API, and a build that leaves it out keeps everything else working (FR-041).
- **Verification**: like every agent-observable change, the checks are scripted against the offline provider for the turn side (message in, history out) with the speech providers faked, plus one live-provider check marked as such for real audio; the plan turns that into concrete steps.

### Superseded surfaces, and the four things that survive them (archived, not deleted)

The call has been drawn three times: as a **bar inside the composer** (amendment `20d97316`), as a
**full screen** (C6), and as a **draggable floating panel** (C9). It is now **one row in the
conversation** (C11). None of the earlier three is a specification for the row, and none is deleted,
because each one recorded a decision worth keeping:

| Retired surface | Why it lost | What it still teaches |
|---|---|---|
| Bar in the composer (`20d97316`) | C6 — a strip of chrome inside the composer is not a call | the state words and their English copy; the narrowing order, which becomes the row's own order of surrender (FR-049) |
| Full screen (C6) | C9 — it hid the conversation, then grew a transcript panel to bring it back | a surface that replaces the conversation has to rebuild it |
| Floating panel (C9) | C11 — the User chose the row: nothing to drag, nothing covered | the drag rules and the remembered position (FR-048, Q9) retire with it; the microphone rule does not |

**Four things survive all four surfaces and must not be re-derived:** the state words and their
English copy (FR-006); the rule that the **microphone indicator is never the element that gives way**
(FR-038); the treatment of `ended`/`failed` — the reason stays, with no five-second timer and no
interrupt control beside it; and the rule that `thinking` is told apart from `reconnecting` by word,
colour **and** marker (FR-042).

**Do not take any retired surface's pixel floor, narrowing order or placement as a requirement for
the row.** The row's own constraints are FR-043, FR-044 and FR-049 and nothing else.

## Open Questions (for the User — ten live, each with a default; the closed ones kept as a record)

Nothing here blocks the plan: each has a decision taken in planning that the plan and the contracts
already follow. Changing one is a change of scope, not a rewrite.

| # | Question | Default taken now | What changes if the answer differs |
|---|---|---|---|
| **Q1** | When the person interrupts, should the agent stop **talking only**, or stop talking **and stop working**? | Stop both: the audio stops and the running turn is cancelled on the existing stop path (FR-027, C3). | "Talking only" keeps a long tool-using turn alive in the background; it needs a way to show that the turn is still running while the person speaks, and the reply's text keeps growing after they cut in. |
| **Q2** | A second tab on the **same** conversation: move the call to that tab, or refuse? (Every *other* conversation and agent now moves — C10.) | Refused: the second tab is told the call is already open here, and the row there says so instead of showing state (FR-005). | Moving would need a rule for what happens to audio already captured in the first tab; refusing is one line of state, and the person can end the call in the tab that has it. |
| **Q3** | Is the 2-second target (SC-001) measured only with hosted speech, with local models excluded? | Yes: hosted only, local measured separately. | Requiring it for a local whisper model means either accepting a slower target or dropping local models from the live path. |
| **Q5** | How visible should "this message was spoken" and "this reply was interrupted" be in the conversation? | A small badge with a tooltip, nothing more (FR-011, FR-023). | Hiding the badges makes the conversation read like typing; making them loud makes voice turns look like errors. |
| ~~**Q11**~~ | Which transport carries the audio path (FR-054)? | **Answered by the User, 2026-11-09 16:35: the WebSocket.** *"kalo liat concern yang disampein SA sama lead sih gw setuju pake websocket dulu aja kalo gitu."* The carrier is **`ws-json-pcm16`** — the path already implemented and tested. The media track stays in the contract as the **reserved** value `media-track`, not as the shipped one, and nothing built for it (setup frames, peer registry, Opus decoder, relay) is written. *The numbers that made this a real choice are kept in `plan.md` so a future decision does not start from zero: str0m +22 crates, 36 s probe build, 583 MB `target`, 1,25 GB peak RSS, Opus decoder + relay required.* | The socket path inherits the work already done: the 46 green client tests, the per-connection lane, the backlog bound and `stall`. What it does **not** do is hide packet loss — no jitter buffer, no PLC, no FEC — so packet loss is a hole in the sound, not a retry. T047's ducking and the 150 ms guard carry the echo and talk-over cases, and the speaker sweep decides whether they are enough (SC-008). |
| **Q12** | Does the reply start being built **before** the person stops talking — a speculative answer to a half-heard question — or does it start when they stop? | **Held; the reply-as-it-is-made work (FR-065 to FR-070) comes first.** Decided by the User on 2026-11-10 06:39: *"level 1 dulu aja deh bro, tapi kalo pas gw tes terus latency nya masih kerasa ganggu baru gw minta level 2"* — so this is a decision to wait, not an open technical puzzle. The Lead's assessment gives three reasons it is not a small extension: (a) recognition is **whole-clip** today (`stt/mod.rs:77-83`), so a partial hypothesis needs a streaming recogniser first; (b) this machine is already CPU-bound with whisper and kokoro competing, and re-decoding a moving window every ~500 ms would starve them; and (c) — the one that decides it — **a speculative turn can act before it is asked to**: a generation built on a half-heard question that is then discarded may already have run `execute_python` or a shell command, so a discarded answer can leave real effects behind. | If the User asks for it, it is its own project: streaming recognition, a speculative turn with an explicit discard rule that forbids side effects before commitment, and a contract for a reply that may be abandoned. The cost of getting (c) wrong is not tokens, it is a half-heard instruction that ran. |
| **Q13** | The spoken-reply toggle leaves the UI (C12). A person who can type but would rather listen now has exactly one way to hear an answer: a live call. Is that acceptable? | **Yes, for this release**: C12 stands, and FR-053 keeps the providers, so the toggle can return without a rebuild. | If not, the toggle is the one manual speech surface worth keeping in the composer — which reopens C12 for that half only, and costs a control the row does not have. |
| **Q14** | Terminal rows: `architecture.md:298` gives an ended/failed row `Start again` / `Try again`, while FR-045 says the row offers **one** action and no second control whose only effect duplicates End. One control or two? | **One**: after a terminal reason the single action becomes the restart, replacing End — the label changes, the count does not. T038 and T094 then test one control in every state. | Two actions means FR-049's never-shrink set grows by one item and the 292 px floor is re-measured — a design and a test change, not a wording change. |
| **Q15** | Is `LIVE_VOICE_SOCKET_BACKLOG = 16` part of the published contract, or an implementation detail a client must not read? | **Published as an upper bound**, exactly as `contracts/live-voice-frames.md:279` states it: 16 audio frames ≈ 1.6 s, then audio is dropped and one `stall` is reported. A bound a client can plan for, not a knob we may halve without notice. | If it is internal, the contract and FR-057's "described in terms another client can meet" must stop quoting the number, and a client cannot size its send buffer. |
| **Q16** | FR-048 is struck *and* kept "because the panel remains the fallback if the row is rejected". Does a struck requirement stay in the document as a fallback, or does the fallback go with the panel? | **Keep it struck**: no task may carry it (T093 is `[SUPERSEDED]`), and reviving it is a new decision with its own delta. | Deleting it makes the archived-surface table the only trace of the drag rules — thinner than what a future panel decision would need. |
| **Q17** | `MANUAL_SPEECH_SURFACES` (`webui/app/screens/chat/Composer.tsx:59`) turns the record control and the spoken-reply toggle back on the day it is set true. Does that surface return **as it was**, or does it need a redrawn design? | **As it was** — FR-053 keeps the code, so no design is owed and Q13's "the toggle can return without a rebuild" stays true. | If it needs a redraw, Q13's default moves from "acceptable for this release" to "a design task is queued", and FR-053's promise narrows to the code only. |
| ~~**Q4**~~ | A hard cap on concurrent live sessions? | **Answered by the User, 2026-11-09 06:52: exactly one — per person, across every conversation and agent (C10, FR-005).** | — |
| ~~**Q6**~~ | What else does the live row carry besides the four required things? | **Closed 2026-11-09 as design's call within a stated bound**: the level of the person's voice and the elapsed time, and nothing that survives FR-049's order of surrender. Reopening it means a fourth thing competing for one line, which is why it is not a question but a constraint. | — |
| ~~**Q7**~~ | Can the person reach the conversation (and type in it) while the call is open? | **Closed by C11 (2026-11-09 16:07): yes, structurally.** A row inside the composer's meta line adds no height and covers nothing, so the conversation and its input are reachable by construction rather than by an answer (FR-049); typed messages stay text-only (FR-025 deferred). | — |
| ~~**Q8**~~ | Is the call a route of its own, or a view over the conversation? | **Answered by the User, 2026-11-09 06:52: neither — it floats over the conversation and there is no route (C9, FR-043).** Superseded in form by C11: it is a row in the conversation, and there is still no route. | — |
| ~~**Q9**~~ | Does the floating panel remember where it was put, between calls or between visits? | **Closed by C11 (2026-11-09 16:07): there is no floating surface, so there is no position to remember.** It was answered while the panel still existed — remembered within one call, reset on the next — and that answer is archived with FR-048. | — |
| ~~**Q10**~~ | When a call is taken by another conversation, is the reason left in the old conversation as a **message**, or only as a notice? | **Closed 2026-11-09: a notice only — a visible reason that is not written into the history**, then the row disappears (FR-051). A message would make a move look like a turn the person did not take, and the conversation is history, not a system log. | — |

## Delta Log — changes made when this draft was resumed (2026-11-08)

**Found at**: `specs/055-live-voice/spec.md`, branch `055-live-voice`, untracked, 255 lines, written
earlier the same day. The branch had no commits of its own, so this draft existed only in the working
tree.

**Kept unchanged**: the Context, all five user stories with their acceptance scenarios, the
interaction-flow diagram, the edge-case list, FR-001 to FR-012, FR-014 to FR-022, FR-024 to FR-026,
FR-028 to FR-034, FR-037 to FR-041, all key entities, SC-002 to SC-009, and the non-goals.
Nothing was deleted.

**Changed** (in order of appearance):

| Change | Why |
|---|---|
| Header: status clarified, "resumed from" note added | Provenance: this file is a resumed draft, not a new specification |
| New `## Clarifications` section (C1–C5) and the code-constraint note | The three open questions are answered; the transport and echo decisions are recorded where a reviewer will look for them |
| FR-013 rewritten | The turn protocol has no token streaming (`llm/sse.rs:1-3`), so the requirement now promises segment-by-segment speech and says why |
| ~~FR-023: "readable in full" → "readable as far as it was produced"~~ **superseded below** | Cancelling the turn (FR-027) means there is no "full" text to keep; the later review delta replaced this wording again, because an abort keeps no partial prose at all |
| FR-027 decided | Interruption stops the audio and cancels the turn |
| FR-035 decided | Chat interface only; Discord and Telegram excluded |
| FR-036 decided, FR-036a added | Cascade only in this release; one seam so a realtime pipeline can come later |
| Three Assumptions lines replaced | They described the questions as open; they now record the answers |
| SC-001 scoped to hosted speech | A local recogniser cannot meet 2 s; measuring it as if it could would make the criterion meaningless |
| New `## Open Questions` section | Five questions for the User, each with the default the plan already follows |

**Review delta (Lead, 2026-11-08, before `speckit_tasks`)**: three corrections, all in this file.

| Change | Why |
|---|---|
| FR-012 reworded | "appear as it is generated" promised token streaming the turn protocol does not have; it now says "as soon as the turn produces its message" |
| FR-023 clarified | the abort path keeps no partial prose (`response.rs:156-163`); interrupting during speech keeps the whole message, interrupting during generation keeps the interrupted mark and the activity trail |
| SC-001 restated | the plan's own budget floors at 2.57 s before the model answers, so 2 s p90 was a target the architecture cannot meet; the gate is now p90 3.5 s / p99 5 s with 2 s p50 as the stretch target |

**Consistency delta (Lead, 2026-11-08, after `@teteh-qa`'s `speckit-analyze`)**: three internal
contradictions removed; no outcome changed.

| Change | Why |
|---|---|
| FR-027 now defers to FR-023's two cases | as written it promised "text produced so far stays", which FR-023's abort case forbids; two requirements cannot answer the same event differently |
| US1 narrative and the flow node say "spoken in segments once its message exists" | they still said "while it is being written", which the non-streaming turn protocol (constraint note) cannot do |
| FR-023's earlier delta line is marked superseded | the log carried two descriptions of the same requirement |

**Consistency delta (Lead, 2026-11-08, from `@teteh-qa`'s analysis)**: two more
corrections, both about testability rather than outcome.

| Change | Why |
|---|---|
| SC-002 states where its clock starts and stops, and splits "silence" from "the turn is cancelled" | with a 250 ms onset guard, "300 ms" had no start point; the guard delays only the message to the server, so the silence is still inside 50 ms |
| the third-party edge case no longer promises speaker attribution | an energy gate on one microphone has no speaker identity; promising it would make an acceptance test nobody can pass. Diarization, if wanted, is a separate specification |

**User decisions of 2026-11-08 17:27 (`muruyung`, via `@bro-pm`)**: four answers, two of
which change this document.

| Decision | What changed here |
|---|---|
| Scope is the full feature: US1 + US2 + US3 | nothing to change — the stories and their priorities already carry all three as P1; the "MVP is US1 only" framing is gone from the delivery notes |
| A thinking indicator when the agent is slow | **new FR-042**; the `waiting` state of FR-006 now has to be *told apart* from `reconnecting` on screen |
| The **agent** decides whether to think longer — no user toggle | **FR-017 rewritten**: short by default, longer when the question needs it, and explicitly "no user-facing switch" |
| A typed message is **text only**; voice belongs to the live session | **FR-018 withdrawn**; US3's independent test and scenario 2 no longer expect a spoken reply for typing; the typing edge case says so too |

Why FR-017 changed and not a new requirement: the earlier wording already said "answer in a form that
suits listening", which the agent interprets per turn. What was missing is that a long, reasoned answer
is *allowed*, and that the choice is never the person's. Why FR-018 was withdrawn rather than narrowed:
the session speaks only the turns it submitted (`architecture.md` §4), so a typed turn's reply could
only be spoken by widening the session into a second submission path — more surface, for a behaviour the
User does not want.

**Verification**: no open-question marker remains in this file — a `grep -c` for the marker phrase returns `0`, and the only remaining pointer to it is this sentence's description of the delta.

## Delta Log — 2026-11-08 21:19 (surface, scope and trigger changed by the User)

Source: the User's message of 2026-11-08 21:19 in the `vizier-dev` workspace, quoted verbatim in the
job that produced this delta. Three changes, applied here in one pass; nothing else was touched.

| Change | Where | What it says now |
|---|---|---|
| **C6 — the call is a screen of its own** | new `### Session 2026-11-08 21:19` in Clarifications · new **US6** · **FR-043 to FR-047** · FR-001, FR-002, FR-006 · Interaction Flow · Assumptions · the archived-bar note | the session opens its own screen in place of the conversation view; the bar-in-composer surface is archived, not deleted |
| **C7 — the interrupt control is deferred to the backlog** | US3 heading + banner · **FR-020**, **FR-021** struck through as deferred · FR-025 reworded so it no longer refers to a control · Interaction Flow · Non-goals | no button in this release; the cost (no escape hatch in a noisy room) is written down and accepted by the User |
| **C8 — barge-in waits for the person to finish** | **FR-019** rewritten · **FR-024**, **FR-027** follow the new trigger · US2 narrative + scenarios (5 → 7) · edge cases · **SC-002** restated · **SC-008** restated · **FR-047** new | the agent keeps talking while the person talks; the end-of-utterance period (default 800 ms) stops the audio and cancels the turn; the onset guard is no longer the mechanism |

**One rule derived from C8, flagged rather than assumed silently.** Because the trigger moved from the
start of speech to its end, the old "a burst too short to be a message stops the audio and sends
nothing" would now cancel a reply for a cough. FR-024 and Story 2 scenario 6 therefore say the
opposite: a sub-word burst **neither** cancels the reply **nor** becomes a message. That preserves the
intent of the original scenario under the new trigger; if the User wants a cough to stop the reply
after all, that is one sentence to change in FR-024.

**Kept exactly as they were** (checked, not assumed): FR-042's thinking indicator and its difference
from `reconnecting`; FR-017's "short by default, the agent decides, no user-facing switch"; FR-018
withdrawn (a typed message is text only); no total duration cap (C-series D8); audio never on the
session broadcast; no new dependency; FR-035's surface limit to WebUI and desktop.

## Delta Log — 2026-11-09 06:52 (the surface floats, and there is one call)

Source: the User's message of 2026-11-09 06:52 in the `vizier-dev` workspace, quoted verbatim in the
job that produced this delta: *"kaya style desktop whatsapp ini, jadi dia floating ga full screen ...
jangan nambah screen baru juga, dia floating nya di screen yang sama aja, tapi bisa di-drag"* and
*"fitur ini cuma bisa aktif di 1 topic aja, kalo dia nyalain ini di topic lain atau bahkan agent lain
maka voice call nya bakal pindah ke topic yang dipilih dan session voice call yang lama otomatis
ditutup"*. Two changes; nothing else was touched.

| Change | Where | What it says now |
|---|---|---|
| **C9 — the call floats, no route** | new `### Session 2026-11-09 06:52` · **US6 rewritten** (title, narrative, independent test, 6 → 9 scenarios) · **FR-043, FR-044, FR-045, FR-046** rewritten · **FR-048**, **FR-049** new · FR-001, FR-002 · edge cases · **SC-010** reworded · wireframe | a draggable panel over the conversation that is already on screen; the chat behind it stays readable and scrollable |
| **C10 — one call in total, and it moves** | Clarifications **C10** · **FR-005** rewritten · **FR-050**, **FR-051**, **FR-052** new · **Q4** and **Q8** closed · **Q2** narrowed · **SC-011** new · edge cases · Assumptions | starting a call anywhere closes the one that was live, with a reason left behind and the turn cancelled — never a refusal, never two microphones |

**Kept exactly as they were** (checked, not assumed): the thinking indicator and its difference from
`reconnecting` (FR-042); "short by default, the agent decides, no user-facing switch" (FR-017); the
typed message being text only (FR-018 withdrawn); **no interrupt button** (C7 — and FR-045 now says so
in the requirement: one action on the panel, and no second control that does the same thing); the
interruption firing at the end of the utterance (C8, FR-019); no total duration cap; audio never on the
session broadcast; no new dependency; WebUI and desktop only (FR-035).

**Two things the plan must now answer, recorded so they are not discovered in code.** First, the panel
is the second surface over a live conversation in this app, and the earlier one is the side-chat panel:
whether they share a positioning mechanism is @mamang-lead-be's call, not this document's. Second,
`LiveVoiceCall.tsx` was written into the task list as a screen; a floating panel is a different
component with different geometry, and the tasks are corrected below rather than left to be
interpreted.

## Delta Log — 2026-11-09 07:17 (a review round: two requirements stood without a carrier)

Not a decision by the User. This round removed two places where the document promised something no task
would ever build — the same class of defect as FR-018, caught before implementation rather than after.

| Change | Why | Where |
|---|---|---|
| **FR-025 deferred** (struck through, joined to FR-020/021) | Its only carrier, **T054**, sits in the deferred US3 (C7). A live requirement whose one task is in the backlog is a release promise with nobody to keep it: QA cannot test it and nobody will build it. Deferring it also makes the release rule readable in one line — **the only way to stop the audio is to stop speaking** (FR-019), not a button and not a keystroke. | FR-025 itself, the typed-while-speaking edge case, the *Interruption* key entity, and `contracts` §`live_voice_interrupt`, which now says the `typed` reason is not sent by this release's client even though both helpers exist (`Session.onTyped`, `forwardLiveVoiceTyped`) because nothing calls them |
| **T053 rewritten** | It claimed the client interrupt path was still to be implemented. It is not missing: `LiveVoiceClient.interrupt(reason)` (`webui/app/lib/liveVoice/session.ts:232 @ webui 8929bbed`) already stops the audio, sends the frame and moves `speaking`/`waiting` to `listening`, with a passing test. What is missing is the **control on the surface this release ships**, because the only renderer today is the archived bar and FR-045 lets the panel carry one action (End) until this item lands. A plan that says "not built" about shipped code misleads in exactly the way the opposite does. | `tasks.md` T053 |
| **T095 corrected** | It said the registry is "keyed by person, not by (agent, conversation)". The `VizierSession` key is what routes inbound frames; deleting it would break delivery. The task now says: **keep the key, add a second index `owner → Arc<LiveVoiceSession>`**, and assert that a frame for the old conversation resolves to nothing after a move. | `tasks.md` T095 |
| **D17 wording** | It read "T054 deleted" while T054 is alive for the typed-message path. It now reads "T054 reused for the typed-message path", so nobody creates a duplicate ID. | `tasks.md` §Delta |

**Nothing was renumbered**: 53 live FR ids, 11 success criteria, 97 tasks (7 of them `[BACKLOG]`) before
and after. `spec.md` gained no requirement: FR-025 lost its force, and that is the only change to the
requirement set.

## Delta Log — 2026-11-09 16:07 (a row, a retirement, and an audio path under evaluation)

Source: the owner of the project, relayed by the User in `vizier-dev` on 2026-11-09, twice — at 16:07
and a correction at 16:16 — quoted verbatim in the jobs that produced this delta: the row over the
panel; the existing STT/TTS surfaces deprecated *in the UI only*, with the Rust code kept; audio on
WebRTC with the transcript on the existing WebSocket, **but open to a cheaper option if one exists**;
the relay shipped by us rather than by a third party; and the audio part able to stand alone for
someone else's client. Four changes, applied in one pass, with one of them written as a question
rather than as an answer because that is what the owner said it was.

| Change | Where | What it says now |
|---|---|---|
| **C11 — one row, and it is the composer's own meta line** | new `### Session 2026-11-09 16:07`, amended 17:31 · **US6 rewritten** (title, narrative, why, independent test, 8 scenarios) · **FR-043, FR-044, FR-045, FR-046, FR-049** rewritten · **FR-001, FR-002, FR-006, SC-010** reworded · **FR-048** and **Q9** struck as superseded · **Q6** reworded · the panel edge case rewritten · the archived-surface note covers all three retired surfaces | the call takes over the composer's meta line at **0 px** added height (surface **B1-1**); it covers nothing, floats over nothing, is not a rail above the composer and has no position to remember |
| **C12 — the manual speech *surfaces* retire** | **FR-053** new · the record and spoken-reply controls leave the composer UI · **FR-003 and FR-041 left exactly as they were**, deliberately | live voice becomes the only speech surface on screen; the providers, tools, flags and build capability stay in the code for another surface later (User, 2026-11-09 16:16), so a build without live voice still has every speech feature |
| **C13 — audio gets a lane of its own; the carrier is the socket (Q11, closed 16:35)** | **FR-054, FR-055, FR-056, FR-057, FR-060** new · **Q11** opened and then closed on `ws-json-pcm16` · SC-001/002/003 scoped to the path that carries the sound · the plan's R1 and the contract's delivery path marked as the shipped path, `media-track` reserved | the sound travels on the conversation's socket as base64 PCM16, everything a person reads stays on the same socket, and the audio is bound to one session; the media option's measured cost stays in `plan.md` as the record of what lost |
| **C15 — third-party access by scoped API key** | **FR-061, FR-062, FR-063, FR-064** new · **SC-013** new · `### Session 2026-11-09 16:35` · contract's Authentication section rewritten | a client that is not Vizier's gets in with a **key that grants live voice and nothing else**; a running session ends when its key is revoked or expires; a key that cannot be narrowed must not be offered at all. The code gap is stated: keys exist (`storage/user.rs:69-81`) but carry no scope (`auth/middleware.rs:200`) |

**Kept exactly as they were** (checked, not assumed): the thinking indicator told apart from
`reconnecting` by word, colour and marker (FR-042); the microphone indicator never being the element
that gives way (FR-038); the reason kept on `ended`/`failed`; **no interrupt control** (C7, and
FR-045 still says the row offers one action); the interruption firing at the end of the utterance
(C8, FR-019); one session in total with reason `moved` (C10, FR-050 to FR-052); no total duration cap;
typed messages being text only (FR-018 withdrawn, FR-025 deferred); and **FR-003 and FR-041, bit for
bit**, which a first reading of "deprecated" would have quietly narrowed.

**Correction, 2026-11-09 16:25 — the echo-cancellation argument is retired, and it changes the
default in Q11.** The first version of this delta made a media transport the leading candidate on one
ground: that the browser's own echo cancellation comes with it. That is wrong, and `webui/app/lib/liveVoice/capture.ts:34`
shows why — `echoCancellation`, `noiseSuppression`, `autoGainControl` and `channelCount: 1` are asked of
`getUserMedia` today, so cancellation is a **capture** feature that any transport gets for free, and the
bracket C5 in this document said as much from the start. FR-054 is reworded to require echo cancellation
of the *feature* and to say explicitly that it is not an argument for a transport; Q11 now states what
WebRTC actually buys (jitter buffer, packet-loss concealment, FEC — resilience the socket lacks) at a
cost that includes a server-side decoder and a relay, and what the socket costs (nothing) and cannot do
(a lost chunk is a hole). **No requirement moved except FR-054's sentence about echo**: FR-055 to
FR-057 and FR-060 were already written as shape, which is why this correction costs one paragraph
rather than a rewrite.

**What this delta deliberately does not decide.** The transport, its stack and its dependencies belong
to the Lead; this document states what has to be true of the audio path (FR-054 to FR-057, FR-060) and
of the published contract (FR-058, FR-059), not which plumbing delivers it. That is Q11, and it is
written as a question because the owner's instruction came with the same escape clause the Lead was
already asking about: *if a cheaper way gets the same result, say so first.* The tasks that follow are
written as *decide on evidence, and record why* followed by *implement against the record*, so the
choice is made once, in one place, with its rationale attached rather than implied.

## Delta Log — 2026-11-10 06:35 (the reply arrives as it is made, and one premise under FR-023 collapses)

Source: the person who used the feature, quoted in full in the 2026-11-10 06:35 session entry. Their
question contained two ideas of different sizes — delivering the reply in pieces, and starting it before
they stop talking — and this delta separates them: the first is written as requirements, the second as
Q12 with "not in this release" as its default.

| Change | Where | What it says now |
|---|---|---|
| **C16 — the reply arrives as it is made** | new `### Session 2026-11-10 06:35` · **FR-065, FR-066, FR-067, FR-068** new · **SC-014** new | a reply need not arrive whole: segments reach the person as they are produced, an interruption stops the remainder, what is stored is what was made, and an ending mid-reply says where it stopped |
| **A premise removed, not a clause rewritten** | **FR-023** (`spec.md:463`) · **FR-012** (`:447`) · **FR-013** (`:448`) · the constraint note (`:38-49`) | FR-023 justified keeping the whole interrupted reply because it was "already stored before its first word was heard". That is true only while replies arrive whole; with FR-065 an interruption can land with a third of the reply produced. FR-012 and FR-013 keep their rule (a segment, never a character) and lose their reason, and the note now says which of its two claims C16 changes and which it leaves |
| **RK-1 becomes an answered risk** | `review.md:239, 254` (the Lead's file — **not edited here**) | RK-1 recorded that live voice is only as live as the model's whole-response latency, with no default taken. FR-065 is the answer to it. Recorded as a **revision of a User decision**, not as a contradiction, and the line itself stays the Lead's to update |
| **SC-001 stops calling itself a gate** | `spec.md:546` | It said "p90 ≤ 3.5 s and p99 ≤ 5 s **is the release gate**". The User decided on 2026-11-09 19:16 that the figure is **recorded, not gated**, because no sample from a real provider exists yet. Fixed here rather than left as a contradiction the next reader has to catch — a correction found while writing this delta, not part of it |
| **Two derived requirements, so the obvious version cannot break a client** | **FR-069** (the finished message still arrives at the end; history keeps one copy) · **FR-070** (the segment frames are a published contract change with its compatibility stated, and a client can tell which revision it speaks) | The pieces are an addition on the way to the same end state, not a replacement for it — and this contract is public (FR-058), so a frame nobody announced is a defect, not an implementation detail |
| **One carrier, and it is provisional** | **T111** appended to `tasks.md` (no task renumbered, no task retitled) | The requirements above have exactly one carrier, marked `[PROVISIONAL]`, which says in its own text that it must not start before the Lead's assessment and the User's answer to Q12. This is rule 8 applied to a promise that has no schedule yet |

**What this delta does not do.** It attaches **no latency number** to a streamed reply — not because one
would be hard to invent, but because the numbers that decide it (whisper's per-clip time, the two LLM
hops measured at 10.66 s and 2.42 s, the first segment's synthesis time) belong to the Lead's assessment
and are not mine to assume. It also does not touch the four fixes already in hand (F1 to F4): those are
implementation against requirements that already exist, and this delta is about the requirement they will
eventually need.

## Delta Log — 2026-11-10 09:00 (the brief was already in the document, and one of its clauses must not be applied)

Source: a scheduled brief of 2026-11-10 09:00 carrying four User decisions of 2026-11-09 16:07 and
asking that they be written here because they were "not yet in the document". They were: three of the
four were written on 2026-11-09 (`aecf38d1`, `518da0fb`), one of them **more narrowly than the brief
says**, and the fourth was **overtaken by the User's own answer twenty-eight minutes later**. So this
delta is a verification with addresses, one sentence loud enough that nobody applies the stale clause
later, and the list of documents that still owe a pass.

**Baseline, locked before reading anything** — `specs/055-live-voice` at `28160187` (branch
`055-live-voice`):

| Measure | Value | Pattern |
|---|---|---|
| task lines | **111** — 46 open, 65 done | `grep -cE '^- \[[ xX]\] T[0-9]{3}' tasks.md` |
| `[BACKLOG]` | **9** | `grep -c '\[BACKLOG\]' tasks.md` |
| `[SUPERSEDED` | **1** task (T093) | `grep -cE '^- \[[ xX]\] T[0-9]{3} \*\*\[SUPERSEDED' tasks.md` — the bare `grep -c '\[SUPERSEDED'` gives **2**, because line 402 also says the word about T093. A prose line is not a task |
| FR ids | **70** used · **5** struck · **65** live | `grep -oE 'FR-[0-9]{3}' spec.md \| sort -u \| wc -l`; struck: `grep -oE '~~\*\*FR-[0-9]{3}' spec.md \| sort -u \| wc -l` |
| SC ids | **14** | `grep -oE 'SC-[0-9]{3}' spec.md \| sort -u \| wc -l` |
| live questions | **6** (Q13 added below) | the Open Questions table |

While this delta was being written the branch moved: HEAD advanced to `45c0b639` (`perf(055)`, touching only `crates/vizier-core/src/agents/agent/mod.rs`). The counts above were re-taken there and are identical — they are stable across both commits, and this line exists so the next reader does not have to reconcile two hashes.

The figures in `review.md:277` (**97 tasks at `dbc50b37`**, 7 `[BACKLOG]`) are four decisions old and
must not be quoted. A bolded bullet can eat an id, which is why the count above uses a pattern the id
cannot escape: `^- \[[ xX]\] T[0-9]{3}`.

### The four items, and where each one already lives

| # | The brief says | What this document already says | Where, and when |
|---|---|---|---|
| 1 | The surface is **one row in the chat**; the floating panel is cancelled | **C11**: the call is the composer's own meta line, at 0 px added height — not a floating panel, not a screen, not a route, not a rail. US6 rewritten; FR-043…FR-046 and FR-049 rewritten around the row; **FR-048 and Q9 struck** (a row has no position to clamp or remember); T093 is the one `[SUPERSEDED]` task and names itself as such | C11 row · `Session 2026-11-09 16:07` · US6 at `:420` · FR-043…FR-049 · `aecf38d1`, amended at 17:31 (`a1afa47b`) |
| 2 | STT and TTS are **deprecated** — check FR-003, FR-041, T014, T015, `sttEnabled` | **C12** deprecates the **surfaces, not the code**: the record control and the spoken-reply toggle leave the conversation's UI (FR-053, T107); providers, tools, flags and build capability stay, because *"incase kedepannya kepake buat UI lain"* (User, 16:16). **FR-003 and FR-041 are therefore left bit for bit** — the brief's instruction to "check" them ends in that check: they got *harder*, not narrower | C12 · FR-053 · T107, T108 · `aecf38d1` |
| 3 | Transport is **WebRTC for audio + WebSocket for chat/transcript**; supersede the `ClientConnection::send` lane, `LIVE_VOICE_SOCKET_BACKLOG=16`, the `stall` frame | **Not applied — see the next section.** The User closed Q11 on the **socket** at 16:35 | Q11 row · C13 · `518da0fb` |
| 4 | "Standalone" is a **public contract**, explained in ≤ 5 plain sentences | **C14/C15** and a four-sentence paragraph at `:288` ("Standalone means the audio capability is a thing a stranger can use without our client…") with the six surfaces that must be public and the five that stay internal; enforced by FR-058, FR-059, FR-061…FR-064, SC-012/SC-013 | `:281-312` · FR-058…FR-064 · `518da0fb` |

**On item 2, one reading is recorded rather than re-asked.** "Deprecated" in the brief is future tense and
unqualified; in this document it means the **surface** is gone, the capability is not. That is not my
inference: it is the User's own qualifier at 16:16, and T108 exists to keep it honest (a build without
live voice still chats, still transcribes uploads, still speaks on request). If a later instruction wants
the *code* gone too, it is a new decision with a task and a migration for anyone who enabled the tools.
Nobody should read this delta as permission to delete the providers.

### The clause that must not be applied: the transport

The brief's third item is the state of the question **before** the User answered it. At 16:35, after the
Lead's cost note, they said:

> *"kalo liat concern yang disampein SA sama lead sih gw setuju pake websocket dulu aja kalo gitu."*

So **Q11 is closed on the socket** (`ws-json-pcm16`), the per-connection lane with
`LIVE_VOICE_SOCKET_BACKLOG = 16` and the `stall` frame is **the shipped path**, not something to
supersede, and `media-track` stays in the contract as a **reserved** value rather than the implemented
one. Writing item 3 as instructed would reverse a later decision by the same person — the one error this
delta exists to prevent. What the brief *is* right about is the shape of the split: **audio on its own
lane, chat and transcript on the ordered broadcast**. That shape is C13 and it shipped; only the carrier
differs from what the brief assumed.

Applying this delta therefore changes **no requirement**: FR-054…FR-057 and FR-060 keep their carrier.
The measured cost of the option that lost (str0m, +22 crates) stays where it is — in `plan.md`, as a
number, not in this document as a preference.

### Carrier audit — every FR this brief touches

| FR | Carriers in `tasks.md` | Verdict |
|---|---|---|
| FR-003 | T014 | carried, done |
| FR-041 | T014, **T108** | carried, T108 open |
| FR-043 | T038, T040, T089 | carried, done |
| FR-044 | T038 | **not carried** — T038 draws the row but not the agent's name (`agentName` has **0** hits in the row at `webui@34e7060`); a carrier is owed, and **T112** is the proposed id while it is not yet in `tasks.md` |
| FR-045 | T038, T090 | carried, done |
| FR-046 | T040, T091 | carried, done |
| FR-047 | T047 | carried, open |
| FR-048 | T093 `[SUPERSEDED by C11]` | retired with the panel — **not** a promise, and the task says so itself |
| FR-049 | T038, T040, T089, T094 | carried, done |
| FR-053 | T107 | carried, done |
| FR-054 | T020, T021, T022 | carried, done |
| FR-055 | T029 | carried, done |
| FR-056 | T101 | carried, open |
| FR-057 | T102 | carried, done |
| FR-058 | T100, T105, T106 | carried, open |
| **FR-059** | no task of its own — **T105 publishes the boundary, T106 proves it** | recorded here so it is not mistaken for an orphan: the conformance test *is* the test of "a stranger needs none of our internals" |
| **FR-060** | none — satisfied **vacuously** on the socket, stated at `:582` | not a promise without an executor: it applies again the moment anything relays audio |
| FR-061…FR-064 | T100, T109, T110 | T109 done, the rest open |

**Result: no FR touched by this brief is carried only by a `[BACKLOG]` task.** The only BACKLOG-only
requirements remain FR-020/FR-021, which C7 deferred on purpose. That is the rule-8 check and it passes —
with FR-059 and FR-060 written down instead of left to look like gaps.

### Documents still owing a pass (owner: `@mamang-lead-be`)

These are **listed, not edited here** — the same convention as the RK-1 row of the 06:35 delta. Each one
still describes the C9 panel as the shipped surface, or asserts a behaviour the spec has since deferred.
Until they are rewritten two sources of truth exist, and `spec.md` wins.

| Document | Lines | What it still says | Should say |
|---|---|---|---|
| `architecture.md` | 47, 141, 257, 298, 470, 482-483, 492 | `LiveVoicePanel` "floats, draggable, one End"; "FR-042 requires the panel to render that as thinking"; a component-table row for `LiveVoicePanel.tsx` with "drag by the header, clamp, re-clamp on resize, position for the life of the call (FR-048)"; "the panel opens here"; "notice on the closed panel" | the row (`LiveVoiceRow.tsx`), the composer's meta line, no geometry, and the reason as a notice on the row |
| `architecture.md` | 133, 334 (I8) | "typing during a session sends `live_voice_interrupt {reason: \"typed\"}` (**the audio stops**, FR-025)"; I8 repeats it as an invariant | FR-025 is **deferred with C7**; `spec.md:481` is the truth — the reply keeps playing, the typed turn queues and is answered as text |
| `architecture.md` | 17 | registry is `LiveVoiceSessions` in `session.rs`, "no separate `registry.rs` module" | **correct — verified**: `ls crates/vizier-core/src/live_voice/` has no `registry.rs`. Nothing to change; listed because the brief named it |
| `tasks.md` | 241-249, 354, 358 | Phase 8 heading "The call **floats over** the conversation (C9)", goal and independent test describing a draggable panel, the demo as "the floating panel" | Phase 8 is US6 v3: the row in the composer's meta line. The delta note at `:401` already says so; the phase did not follow |
| `plan.md` | 238, 259 | Phase E goal "the floating panel exists and can be dragged"; exit criterion "opens the floating panel and it drags within bounds" | the row opens with 0 px added and gives up detail in FR-049's order |
| `plan.md` | 255 | step 3 ends with a leftover "(default: no — the conversation view is what the call replaces)", contradicting the clause before it | drop the leftover; FR-025 is deferred, nothing stops the audio |
| `quickstart.md` | 137 | the e2e row promises "the interruption path and the floating panel" | the interruption path and the row |
| `review.md` | 274 (C-15) | "the surface is a **floating panel**… FR-048 (drag, clamp, resize, position) and FR-049 new… C-13 and C-14 are superseded, not withdrawn" | C-15 is itself superseded by C11: FR-048 and Q9 retire, FR-049 is about the row, and what survives from the panel work is the state words and the never-give-up-the-microphone rule |
| `review.md` | 277-288 | "Canonical baseline… **97 tasks at `dbc50b37`**, 7 `[BACKLOG]`" | **111 tasks, 9 `[BACKLOG]`, one task superseded (T093) at `28160187`**, with the patterns in the table above |
| `analysis.md` | 226 | "T089..T097 cover the panel and the move" | T089…T097 cover the row and the move; T093 is superseded |
| `diagrams.md` | 168 | history list: bar → screen → draggable panel → rail → row | **correct as history** — leave it |

**Status of this table at 2026-11-10 10:42 — read it before acting on it.** Six of its eleven rows are now
**closed**, three of them by the pass that wrote this note: `plan.md` 238/259 (the Phase E goal, the exit criterion
and the leftover "default: no") and `quickstart.md:137` are fixed in the delta below, and `architecture.md`
47/141/257/298/470/482-483/492 and 133/334 were rewritten by the Lead — the component table now names
`LiveVoiceRow.tsx` with "no route, no panel, no drag", and the typing sentence now says the reply keeps playing.
`review.md:274` (C-15) carries its supersession note. **Still open: `tasks.md`** (the Phase 8 heading and goal,
the delivery notes at `:354`/`:358`, and the two task lines at `:258`/`:259`) — the Lead's file, listed again with
line numbers in the 10:42 delta. Do not re-fix a closed row from this table; it is a record of what was owed,
not a work list.

**Tests for withdrawn behaviour: swept, and two results.** No `LiveVoicePanel` component, unit test or
e2e file remains (`grep -rln LiveVoicePanel crates webui docs` returns nothing), so nothing is green
against the panel. The one surviving "typed" assertion is
`crates/vizier-http/tests/live_voice_barge_in.rs:268`, which checks that the three interrupt **reason
strings** are distinct on the wire — a vocabulary test, not a behaviour test, and it should stay: the
value is part of the contract even while FR-025 is deferred. It must not be cited as evidence that
typing stops the audio.

### One open question, and it is a cost, not a decision

Q13 is added to the table: removing the spoken-reply toggle leaves a person who can type but would
rather listen with exactly one way to hear an answer — a live call. The default is "yes, that is
acceptable for this release", because FR-053 keeps the code and the toggle can come back without a
rebuild. It is written down so the trade-off is visible, not to re-open C12.

## Delta Log — 2026-11-10 09:27 (the copy stays English, and QA verifies without a database)

Two decisions from the User, relayed by `@bro-pm` at 09:31. Neither changes a requirement's meaning;
both close a question that would otherwise be answered differently by whoever got there first.

**1. The interface is English, and stays English.** *"Bahasa inggris aja UI nya"* — recorded at FR-006,
because that is where the eight state words live. The decision is not cosmetic and not a preference
about tone: `thinking` is 47 px and `agent sedang berpikir` is 130 px, so translating the copy re-measures
the row that FR-049 pins at 24 px with a fixed order of surrender. There is therefore **no copy pass, no
width re-measure, and no delta** — and a future reader who wants Indonesian has to say so and accept the
re-measure, rather than treating it as a find-and-replace.

**2. QA verifies without a database.** *"Nggak sama sekali"* — no `mysql-local`, no direct reads of a live
store; fixtures are the only data source. Recorded at Measurable Outcomes rather than in `checklists/`,
because it constrains **what can be claimed**, not how a test is written: a criterion that can only be
shown against real stored data is marked **"cannot be checked"**, and that label is a result. The
alternative — substituting an assumption for the data and calling it a pass — is the failure mode this
spec has already paid for twice.

**What this delta deliberately does not do.** It does not reopen the surface. The row is **B1-1**, locked
by the User on 2026-11-09 17:31, rendered, verified by two parties, and implemented in
`webui/app/screens/chat/LiveVoiceRow.tsx`. A brief asking the designer for "three design options" was
circulated on 2026-11-10 09:31 and is **declined here**: the surface is not open, and drawing options for
a decided surface costs the designer time with no receiver. What remains for design is narrower and
already named — relabel the four frames that still call a retired surface `CANONICAL`, and fill the
"drawn as" column for the rows that genuinely need a drawing. `listening` and `interrupted` are
explicitly **not** required to be drawn now.

**Still owed, unchanged and not mine:** the `architecture.md` and `review.md` pass listed under
"Documents still owing a pass", owner `@mamang-lead-be`, queued behind the gate that the User is waiting
on. `architecture.md:17` stays off that list: the registry really is `LiveVoiceSessions` in `session.rs`,
and there is no `registry.rs` — that line is correct and must not be "fixed".

## Delta Log — 2026-11-10 09:44 (FR-063 splits: revocation is timed, expiry is not)

Baseline this delta was written against: `vizier@2acd9889` · `tasks.md` 46 `[ ]` / 65 `[x]` / 9 `[BACKLOG]` ·
71 unique `FR-###` ids (65 + `FR-036a` + `FR-065`…`FR-070`). Perintah: `git log -1 --oneline`;
`grep -cE '^- \[ \] T[0-9]{3}' tasks.md`; `grep -oE 'FR-[0-9]{3}[a-z]?' spec.md | sort -u | wc -l`.

**Which door this came through.** `@mamang-lead-be`, 2026-11-10 09:40, on the path the User ratified at 09:36:
a spec change caused by a downstream finding enters through the Lead. The finding is `@teteh-qa`'s (`GI-03`),
verified in code by the Lead: `crates/vizier-http/src/topic.rs:410-421` ends a lapsed key's session when a frame
arrives, and no timer exists anywhere to do it sooner.

**The decision that was mine: split, not footnote.** A footnote would have left one MUST carrying two events
whose promises cannot be kept together, with the weaker half borrowing the stronger half's credibility — the
failure this spec has already paid for once (FR-025, deferred because its only carrier was in the backlog).

| Requirement | Event | Promise | Carrier |
|---|---|---|---|
| FR-063 | revoke, rotate | within **1 s**: reason `key_revoked`, mic released, turn cancelled — `SC-013` | `T110` (round 1, in flight) |
| FR-063a | expire | at the **next client frame on the open connection**: reason `key_expired`, mic released, turn cancelled — **no time bound** | `T110`, `GI-03` |

`FR-063a` is a variant id on the convention `FR-036a` already set: no renumbering, so the references that
already exist keep pointing where they pointed — six of them at `2acd9889`, by
`git grep -o 'FR-063' 2acd9889 -- 'specs/055-live-voice/' | wc -l` (two in this file, three in the contract,
one in `tasks.md`).

**Two facts measured before writing this delta, because both change what could honestly be promised.**

1. **There is no idle stop yet, so "a silent session ends with `idle` after 3 minutes" is not true today.**
   `git grep -n 'EndReason::Idle' -- 'crates/**'` returns one hit, a test fixture (`live_voice/frame.rs:420`),
   and `T062` — the idle timer of FR-029 — is `[ ]` (`tasks.md:213`). So the honest statement of the gap is not
   "the ending is mislabelled `idle`". It is: **while the client stays silent the session outlives its key, and
   nothing ends it.** When `T062` lands, a silent session may be ended by FR-029's timer with reason `idle` —
   true (the person was silent) but silent about the key. That interaction is recorded here as an **ordering
   dependency**, not as a promise of this release, and it is re-read when `T062` lands.
2. **`T110`'s own test line already asked for the one second on both events** (`tasks.md:119`: *"the reason
   within 1 s (SC-013)"*), while `SC-013`'s sentence is about revocation only. With the split, the expiry half is
   asserted on the frame, not on the clock. That edit belongs to the Lead.

**Documents that now disagree, and their owner** (the orphan class this repo has been bitten by twice):

| Where | What is now wrong | Owner |
|---|---|---|
| `contracts/live-voice-frames.md:356` — lifecycle table, `key expired` row | says the next request is refused and a live session ends; silent about *which* frame and about the absence of a time bound, while the header of that section quotes FR-063's "rather than being noticed at the next connection" | `@mamang-lead-be` |
| `tasks.md:119` (`T110`) | one 1 s promise covering both events, and no `(FR-063a)` tag | `@mamang-lead-be` |
| this file, C15 row at `:852` | lists FR-063 only | left as history; this delta supersedes it |

**Found while verifying, for the Lead's gate — deliberately not written into a requirement.** Revocation is not
re-checked per frame: `key_lapsed` (`topic.rs:67-74`) reads `principal.method`, stamped once when the connection
opens (`topic.rs:243`), and revocation is only pushed by `delete_api_key` → `end_owned_by_key`. So after a
revoke ends a session, the same open socket can send `live_voice_start` again — the principal still carries the
key id and its permissions — and FR-064 says a request with a revoked key is refused *before any session state
is created*. **Not proven:** no test was written. A test that revokes and then starts again on the same socket
settles it in one run, and it is the natural first line of `GI-04`.

**Still owed, and not mine:** the `architecture.md` and `review.md` pass, owner unchanged.


## Delta Log — 2026-11-10 10:42 (the sweep: the prose still sold the panel, and every sentence is named)

Source: the scheduled brief of 2026-11-10 09:00, re-read at 10:42. The first pass over it (`73d9a09d`, 09:00)
answered its four items with addresses, struck the transport clause, audited carriers and listed the documents
that still owed a pass. What that pass did **not** do is change a sentence: three files I own still described the
call as a panel in their own voice, so a reader who trusts prose over ids would have rebuilt the retired surface.
This pass is that sweep, plus a re-audit at the commit it lands in. It **adds no requirement, withdraws none, and
renumbers nothing**.

**Baseline, locked before reading anything** — `specs/055-live-voice` at `cf663414` (branch `055-live-voice`):

| Measure | Value | Pattern (the one used, so it can be re-run) |
|---|---|---|
| task lines | **111** = **64** `[x]` + **47** `[ ]` | `grep -cE '^- \[[ xX]\] \*{0,2}T[0-9]{3}' tasks.md` |
| `[BACKLOG]` **tasks** | **7** (`T051`–`T057`) | `grep -cE '^- \[[ xX]\] .*\[BACKLOG\]' tasks.md` — the bare `grep -c '\[BACKLOG\]'` gives **9**, because two prose lines say the word. A prose line is not a task |
| `[SUPERSEDED]` | **1** (`T093`) | `grep -c 'SUPERSEDED' tasks.md` = **2** → one is the prose note at `:402` |
| `[PROVISIONAL]` | **1** (`T111`) | `grep -c 'PROVISIONAL' tasks.md` |
| FR ids | **72** = **67** live + **5** struck (`FR-018`, `FR-020`, `FR-021`, `FR-025`, `FR-048`) | live `grep -cE '^- \*\*FR-[0-9]{3}[a-z]?\*\*:' spec.md`; struck `grep -cE '^- ~~\*\*FR-[0-9]{3}[a-z]?\*\*:' spec.md`. **The colon is load-bearing**: without it, cross-reference bullets inside delta logs (`- **FR-012** (\`spec.md:447\`) says…`) count as definitions and inflate the total to 74 |
| SC ids | **14** | `grep -oE 'SC-[0-9]{3}' spec.md \| sort -u \| wc -l` |
| live questions | **6** at baseline (`Q1`, `Q2`, `Q3`, `Q5`, `Q12`, `Q13`); **10** after this delta adds `Q14`–`Q17` | `grep -cE '^\| \*\*Q[0-9]+\*\*' spec.md` |

### The prose, swept (ten sentences, three files, all mine)

| File | What it said | What it says now | Why it mattered |
|---|---|---|---|
| `spec.md` FR-006 | "`off` is the absence of the **panel**" | "the absence of the **row**, not a state the row renders" | The one sentence in the requirement set that named the retired surface — inside the requirement every test of the state word reads |
| `spec.md` FR-025 (deferred) | "the composer surface is frozen **until the User picks a panel**, so a live requirement would have been a promise with no one to keep it" | "the row carries exactly one action, End (FR-045), so a live requirement would have been a promise…" | The *reason* FR-025 was deferred was kept, the *premise* was stale: the surface is picked, and what still keeps FR-025 out is FR-045's one action |
| `spec.md` FR-050 | "the new **panel** opens… the old **panel** disappears" | "the **row** appears… the old **row** disappears" | A move is described in prose nowhere else; an implementer would have looked for a panel to close |
| `plan.md` gate table | "**The floating panel's** states … are drawn … The archived bar is not the design; **the screen is** (spec C6)" | "The **row's** states … the archived bar, screen and panel are not the design; **the row is** (spec C11, B1-1)" | The constitution gate cited C6 — two decisions behind — as the design of record |
| `plan.md` structure | "the **floating-panel** wireframe" | "the live-row wireframe" | It describes what `diagrams.md` actually contains |
| `plan.md` structure comment | "the composer bar … is archived (spec C6): **the call is a screen**" | all three retired surfaces named (C6, C9, C11); the row is the call | It asserted the *screen* was current — the surface C9 replaced |
| `plan.md` Phase E goal | "the **floating panel** exists and can be dragged" | "the row exists in the composer's own meta line at 0 px added height" | The phase goal is what an implementer reads first, and it asked for a drag |
| `plan.md` Phase E step 3 | "the typed turn queues … **on the same path as any other interruption**… typing stays reachable because the conversation is still **behind the panel** (FR-049, Q7) (**default: no — the conversation view is what the call replaces**)" | the typed turn takes **no** interruption path and is answered as text; typing stays reachable **by construction** | Three errors in one sentence: FR-025 is deferred (typing interrupts nothing), Q7 is closed (not open with a default), and the leftover "default: no" contradicted the clause before it |
| `plan.md` Phase E exit | "the control opens the **floating panel** and it drags within bounds" | "opens the row at no added height and the conversation does not move" | An exit criterion is a test; this one would have failed a correct row |
| `quickstart.md` e2e row | "the interruption path and the **floating panel**" | "the interruption path and the **live row**" | The one line a person follows by hand |

### Carrier audit — the FRs this round touches, at `cf663414`

| FR | Carriers | State | Verdict |
|---|---|---|---|
| FR-003 | T014 | `[x]` | carried |
| FR-041 | T014, **T108** | `[x]`, `[ ]` | carried — T108 is the harder half (two speech paths coexist) |
| FR-006, FR-038, FR-042 | T038, T039 | `[x]` | carried |
| FR-044 | T038 | `[x]` | **not carried** — the row exists, the agent's name does not; a carrier is owed (T112 proposed). Corrected at 12:52 |
| FR-043, FR-046, FR-049 | T038, T040, T089, T094 | `[x]` | carried |
| FR-045 | T038, T090 | `[x]` | carried; **T052/T053 (`[BACKLOG]`) add a control to it — they are not its carrier** |
| FR-047 | T047 | `[ ]` | carried, open — the release still owes SC-008 |
| FR-048 | T093 **`[SUPERSEDED]`** | `[ ]` | **retired, not deferred**: the task says so itself; the FR stays struck as the archived fallback (Q16) |
| FR-050, FR-052 | T095, T097 | `[ ]` | carried, open |
| FR-051 | T096 | `[ ]` | carried, open |
| FR-053 | T107 | `[x]` | carried — the surfaces are gone from `Composer.tsx` behind `MANUAL_SPEECH_SURFACES = false` (`:59`), and the code is not |
| FR-054, FR-055 | T020, T021, T022 / T029 | `[x]` (T021 `[BACKLOG]`) | carried by done tasks; **T021's backlog state concerns an *alternative* audio shape, not FR-054 itself** |
| FR-056, FR-057 | T101 / T102 | `[ ]` / `[x]` | carried |
| FR-058, FR-059 | T100, T105, T106 | `[ ]` | carried — FR-059's carrier is T106 (the conformance test), recorded at `:985` |
| FR-060 | — | — | satisfied **vacuously** on the socket, stated in the requirement itself; not an orphan |
| FR-061 … FR-064 | T100, T109, T110 | `[ ]`, `[x]`, `[ ]` | carried |

**Result: no FR in this set is carried only by a `[BACKLOG]` task.** The only backlog-only requirements in the
document remain FR-020 and FR-021, which C7 deferred on purpose and whose US3 banner says so.

### Documents still owing a pass (owner: `@mamang-lead-be`)

The 09:00 list is now mostly closed by the Lead's own passes, and one row closed **during this pass**: `review.md`
C-15 (`:274`) now carries "Superseded by C11 … this row is now history, not a promise". `architecture.md` no longer
names the panel — `:298` is the row (`no route, no panel, no drag`) and the "typing stops the audio" sentence is
corrected to "the reply keeps playing". What is left:

| Document | Line | What it still says | Should say |
|---|---|---|---|
| `tasks.md` | **241–249** | Phase 8 heading "The call **floats over** the conversation (C9)", goal and independent test describing a draggable panel | Phase 8 is US6 v3: the row in the composer's meta line — the note at `:401` already says so; the phase did not follow |
| `tasks.md` | **354, 358** | "The demo is the **floating panel** (US6)"; "US1 → demo → **US6 (the floating panel)**" | the row |
| `tasks.md` | **258, 259** (T096, T097) | "the old **panel** shows it until it disappears"; "no segment … reaches the new **panel**" | the old **row** / the new **row** — prose inside two live tasks, so an implementer reads it as instruction |
| `tasks.md` | **432** | history line naming `LiveVoicePanel.tsx` as the old target | **correct as history** — leave it |
| `diagrams.md` | 168 | history list: bar → screen → draggable panel → rail → row | **correct as history** — leave it |
| `architecture.md` | 17 | registry is `LiveVoiceSessions` in `session.rs`, "no separate `registry.rs` module" | **correct — do not "fix" it**: `ls crates/vizier-core/src/live_voice/` has no `registry.rs` |

### Tests that would be green against withdrawn behaviour

| Where | What it is | Keep? |
|---|---|---|
| `webui/app/lib/liveVoice/session.test.ts:125` | `it.skip('a typed message interrupts…')`, skipped with the reason inline (FR-025 deferred, C7) | **keep skipped** — it is the marker that the path exists and nothing calls it |
| `crates/vizier-http/tests/live_voice_barge_in.rs:268` | the three interrupt **reason strings** are distinct (`barge_in`, `button`, `typed`) | **keep** — a vocabulary test, not a behaviour test; it must not be cited as evidence that typing stops the audio |
| `webui/app/screens/chat/Composer.tsx:59` | `MANUAL_SPEECH_SURFACES = false` switches the record control and the spoken-reply toggle off | **keep** — this *is* C12's implementation, and FR-053 requires the code to stay |
| any `LiveVoicePanel` component, unit test or e2e | `grep -rln 'LiveVoicePanel' webui crates docs` → **0 files** | nothing green survives the panel |

### The third clause of the brief, again, with the numbers on disk

The brief's third item — audio on **WebRTC**, chat on the WebSocket, superseding `ClientConnection::send`,
`LIVE_VOICE_SOCKET_BACKLOG = 16` and the `stall` frame — is the state of the question **before** the User answered
it at 16:35 (`73d9a09d` carries the quote). It is **not applied**, and the carrier is verified at this commit rather
than remembered: `crates/vizier-core/src/live_voice/mod.rs:35` (`pub const LIVE_VOICE_SOCKET_BACKLOG: usize = 16`),
`crates/vizier-http/src/topic.rs:1053` (the drop decision), `crates/vizier-http/tests/live_voice_delivery.rs:44`
(`assert_eq!(LIVE_VOICE_SOCKET_BACKLOG, 16)`), `contracts/live-voice-frames.md:279` (the published bound: 16 audio
frames ≈ 1.6 s, then dropped, one `stall` reported). Replacing that lane on the strength of a brief written before
the answer would reverse the decision of the same person, 28 minutes later.

What the brief is **right** about is the shape, and the shape is what shipped: **audio on a droppable lane of its
own, everything a person reads on the ordered broadcast.** FR-054, FR-055 and FR-056 already say exactly that, and
Q15 asks the one thing the sweep found unstated — whether that bound is public.

### Open questions added by this pass

Four, all in the table at `## Open Questions`, each with the default I would take: **Q14** one action or two on a
terminal row (`architecture.md:298` labels a restart while FR-045 allows one control); **Q15** whether
`LIVE_VOICE_SOCKET_BACKLOG = 16` is a published bound or an internal knob; **Q16** whether a struck requirement
(FR-048) may stay in the document as the archived fallback; **Q17** whether the manual speech surface returns
as it was when `MANUAL_SPEECH_SURFACES` is set true. None of them changes a requirement's meaning, and each names
what would change if the answer differs.

### What this delta does not do

It does not edit the Lead's files (`architecture.md`, `review.md`, `tasks.md`): the rows above are **listed, not
edited**, the same convention as the RK-1 row of the 06:35 delta. It does not reopen the surface — B1-1 is locked
by the User, rendered (`055-live-voice-screen.figcraft@02ddd7b4`) and implemented (`LiveVoiceRow.tsx`), so the
"three design options" brief stays **declined** (`73d9a09d`, 09:27) and this pass does not revive it. And it decides
no stack and no dependency: the transport is the User's answer, and the numbers of the option that lost stay in
`plan.md` as numbers.


## Appendices

| File | What it is | The gate it carries |
|---|---|---|
| `state-matrix.md` | **The state matrix of the live row**: one row per state the row can be in, carrying what draws it (`digambar apa`, design), what data it needs (`butuh data apa`, data), when it is triggered (`dipicu kapan`, analysis) and what passing looks like (`lulusnya kayak apa`, QA). Fifteen rows, three desks, one file — it is an appendix of this specification, not a second document. | A UI surface of this feature is **not done** until this table exists, covers every state FR-006 names, and its `approved:` line carries a date and a name. A row marked `n/a` is a **decision**, not a gap: it must say why and when the decision was taken, or it is an open question with an owner. |

**Pin rule — three refs, not two.** The matrix is a test checklist only in a revision where **`spec_ref:`,
`design_ref:` and `code_ref:`** all resolve: a commit that exists, a file whose bytes still hash to the recorded md5,
and a `webui` pin whose line numbers the table was actually read against. In any other revision it is a draft, and no
"failed" may be written against it, because the thing it would fail is text that no longer exists.

The third ref is not symmetry. Measured on 2026-11-10: the matrix carried **thirteen** `session.ts` line ranges, and
**no single commit made all thirteen true at once** — nine were read at `webui@510ace84` (the pin when the data
column was written at 08:57: `:97`, `:82`, `:27-46`, `:249-255`, `:250-253`, `:256-259`, `:373`, `:566-567`,
`:540-586`) and four at `webui@08cf067a` (`:109-111`, `:180-192`, `:57`, `:219`), with two bare numbers (`:232`,
`:331`) from the same later pin. One row, `LV-ST-LISTENING`, carried **both pins in one line**. When `webui` then
moved twice in eight minutes (`2be9c377` 11:19, `8929bbed` 11:24), every one of them pointed somewhere else — and
the file looked freshly verified, because every hash in it was correct. A pin that names the spec but not the code
is a claim about a document and a silence about the thing that moves.

**A pin is a date, not a guarantee.** `webui` moved four times in three hours on the day this rule was written, so
`code_ref` records where the numbers *were* checked, not that they are still right. The check is one command, and it
belongs before a signature rather than in a promise:

    git ls-tree HEAD webui | awk '{print $3}'   # differs from code_ref→ the table is stale, re-read it before signing

**Counted at this commit** (so a later reader does not have to trust it, and can re-run it). FR-006 names **eight**
states. The matrix carries **15** rows (`grep -c '^| LV-ST-' state-matrix.md`), and the extra seven are not padding —
each is a branch the code has and FR-006 does not name:

| Rows | Why it is a row |
|---|---|
| 9 session states — `off`, `connecting`, `listening`, `waiting`, `speaking`, `interrupted`, `reconnecting`, `ended`, `failed` | the row's own states; `frames.ts:18-27` has **9** values and these are they. `listening` and `interrupted` are rows of their own even though the sheet does not draw them |
| `!capability` | one row for the **three** `live_voice=false` reasons (`voice`, `recogniser`, `none`) because the code draws **one** non-button pill for all three — the `indistinguishable` case, flagged rather than merged away |
| `overflow` | geometry, not a session state: the row giving up its own detail in FR-049's order at a narrow pane |
| `no-speech` | **two** thresholds (utterance under 250 ms, and a session that heard nothing) on one screen |
| `error-forbidden` (403), `error-transient` (5xx/timeout), `error-session-active` | three responses FR-006 compresses into the single word `failed`; splitting them is what makes the row's copy testable |

The sheet draws **7** of the row's states (`9679`: `idle` + `R2` + `S1`–`S5`), one partially (`9462` S3), and the
other seven are `n/a` with a dated reason — which is what the third column of the table above is for.

## Delta Log — 2026-11-10 12:25 (the row must name the agent, and FR-049 has to say where it goes)

Found while the row was being used, not while it was being designed: `@teteh-desener` read `spec.md:565`
and `webui/app/screens/chat/LiveVoiceRow.tsx` side by side and noticed the row **does not show the agent**
— it is implied by the conversation header. FR-044 is unambiguous: *"The **row** MUST show, for the whole
session: the agent the call is with, …"*, with no clause allowing another surface to stand in.

**This is an implementation defect, not a specification one, and the carrier is dishonest.** `T038` is
already `[x]` (`tasks.md:150`) and its own text promises *"It shows the agent … (FR-044)"*, while
`LiveVoiceRowProps` carries no agent at all (corrected at 12:52: the interface has seven members — `view`, `capability`, `missing`, `elapsedSeconds`, `onStart`, `onEnd`, `onRestart` — the point being that none of them is an agent) and the render body
(`:205-260`) holds the state word, the marker, the microphone, the reason/transcript/waveform slot, the
timer and End — and no agent. That is the same class as `T026` earlier today: a tick that disagrees with
its own sentence. **Un-ticking it is the Lead's file, not mine**, and it is recorded here rather than
done here.

**Why the fix is a clause in FR-049 and not an amendment to FR-044.** The cheap way out is to weaken the
requirement to match the code — accept the header as "showing the agent". It was refused, on a rule this
document already lives by: **FR-038 rejects a tooltip as "show"** for the microphone indicator, so a
header cannot be "show" for the agent either. One standard for two equivalent things, or neither. The
real cost of satisfying FR-044 is **width**, and width is the one thing a 24 px line cannot invent — which
is exactly what FR-049 governs. So the agent's name joins **step (1)** of the surrender order, with the
model chip: chip to 96 px then gone, name truncated with its tail ellipsised, and both dropped before the
waveform. It is deliberately **not** in the never-shrink set with the microphone, the state word, the
elapsed time and End — a long agent name would eat the transcript, and the transcript is the only element
on that line that carries words.

**What this delta does not do.** It does not add a task (the carrier is `T038`, whose tick has to come off)
and it does not decide the geometry — the designer measures the width, the Lead's implementer builds the
element, and both are downstream of this clause. It also leaves `Q6` alone: the level meter is still
Q6's business, and the reason it disappears after the first word is **V9** (`view.transcript` is only
cleared in `start()`, `session.ts:196`), a separate bug already in the queue.

## Delta Log — 2026-11-10 12:40 (the floor was below the row's own content, and the chip goes before the name)

`@bro-pm` relayed `@teteh-desener`'s 12:34 measurement and asked for one written decision. Two things came
out of it: FR-049's floor was not merely tight but **impossible**, and step (1)'s order could be read two
ways.

**The arithmetic, from the constants and not from the sheet.** The old floor was written in five places —
`:209`, `:440`, `:490`, `:570` and `:696` — and all five now carry the new one; the old figure survives exactly
once, as history inside FR-049 itself. The source
is `webui/app/screens/chat/LiveVoiceRow.tsx` at `34f47d27`'s tree: `ROW` (`:32`) gives state word 70,
elapsed time 34, End 54, gap 8; the live container (`:211`) carries `height`, `fontSize` and `gap` and
**no `paddingInline`** — the 10 px pad belongs to the idle pill (`:180`) and the terminal button (`:263`),
which is why the designer's figures reproduce and why padding does not inflate them. Six flex children
(state word, marker, microphone group, middle slot, timer, action) carry **five** 8 px gaps = 40; with the
marker at its measured 20 px the fixed set is **276 px**, without a visible marker **256 px**. The old
floor of 240 was therefore **36 px short of the thinking case and 16 px short of the quiet one**: the row
could not be rendered at its own floor even with the middle slot empty.

**One correction to a figure that was circulating.** `248 px` is not reachable and should not be quoted
again: the marker's `<span>` is in the DOM in every state (`:224`, empty string when the state is neither
`waiting` nor `reconnecting`), so the gap count stays five whether the dots are drawn or not. "No marker"
is **256 px**. A number nobody can reproduce from the source is not a measurement.

**The decision: the floor rises to 292 px**, and it is the only one of the three offered that does not
weaken a requirement.
- *Raising the floor* costs nothing that exists. At the minimum window (900 px) the pane is far above 292
  already, so the floor still guards exactly one thing — a pane narrowed by surrounding chrome — but now
  with a number the row can meet: 276 + 16 px of middle slot.
- *Letting a never-shrink element shrink* is refused because that set is named requirement by requirement:
  the microphone indicator (FR-038), the state word (FR-042), the elapsed time and the one action (FR-045).
  A row that drops the microphone indicator in order to fit is the defect FR-038 was written to prevent.
- *Making the thinking marker optional at narrow widths* is refused because **FR-044 already puts it in the
  row's minimum** — "the thinking indicator of FR-042 when the agent is working". Optional-ising it would
  amend FR-044 through the back door, the same move that was refused this morning when it was proposed for
  the agent's name and the header. **FR-044, not taste, is what forces 292 rather than 256.**

**Step (1) now reads chip-then-name, one way only.** The previous sentence — "the agent's name and the
model chip shrink and then go, … before either is dropped whole" — allowed the reading that both shrink
together and either may be dropped first, which is precisely how the optional element could be held while
the required one went. It now reads: the chip to 96 px and then gone; the agent's name shortened **only
once the chip is gone**, and last of the two to go. The reason is written into the requirement so the next
reader does not have to re-derive it: the name is FR-044's requirement, the chip is Q6's optional extra.

**What this delta does not do.** No task, no code, no design. `T038`'s tick is still dishonest and still
the Lead's file, as recorded at 12:25. The geometric carrier is `T094`'s narrow-window test, which now
tests 292 rather than 240 — a one-number change to the Lead's file, **recorded here and not made here**.
`T038` itself does not move: the row's nearest element to this is the agent's name, whose width the
designer measures.

## Delta Log — 2026-11-10 12:52 (FR-044 was marked carried by a task that does not carry it)

`@mamang-lead-be` read the carrier map and found it saying something the code does not: **FR-044 is marked
`carried, done` through T038**, while T038's own sentence promises *"It shows the agent … (FR-044)"* and the
row it produced has no agent in it.

**The three measurements, at `eb927905` / `webui@34e7060`.** (1) `tasks.md:150` is `- [x] T038` and its text
contains "It shows the agent, the FR-006 state word …, (FR-044)". (2) `grep -c agentName
webui/app/screens/chat/LiveVoiceRow.tsx` → **0**; `LiveVoiceRowProps` (`:17-29`) has seven members — `view`,
`capability`, `missing`, `elapsedSeconds`, `onStart`, `onEnd`, `onRestart` — and no agent among them. (3) `LiveVoiceControl` receives `agentId` and
nothing else about the agent (`Composer.tsx:471-476`), while the display name it would need is already in
the composer's scope: `routes/chat.tsx:103` `const agentName = agent?.name ?? agentId`, passed down at
`:387`. So the work is not hidden behind a component that already has the data — it was never wired, and the
name exists one level above where the row is rendered.

**Two rows corrected, not one.** The standalone table at `:982` is the one that was found; the carrier audit
further down grouped *"FR-006, FR-038, FR-042, FR-044 | T038, T039 | `[x]` | carried"*, repeating the same
claim in a second place. Both now say FR-044 is **not** carried and that a carrier is owed. T112 is named
only as the **proposed** id: an id that is not in `tasks.md` is not a carrier either, and this delta exists
precisely because the document said "done" about work nobody had taken.

**What this delta does not do.** It does not add the task: `tasks.md` is the Lead's file and this round's
standing instruction is that I do not write it, so the text for T112 is handed over in the thread instead.
It does not re-decide the floor — but T038's line still reads "Floor 240 px", which is the 12:40 decision's
consequence inside the Lead's file, recorded here and not edited here.


