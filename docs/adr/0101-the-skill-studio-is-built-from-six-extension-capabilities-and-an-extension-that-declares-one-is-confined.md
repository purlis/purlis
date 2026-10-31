# The Skill studio is built from six extension capabilities, and an extension that declares one is confined

**Accepted 2026-10-20**, from the operator's spec #2762 (M65, Skill studio), whose decisions were
grilled with the operator on 2026-10-20 (decisions 8, 22, 27, 43, 45, 48, 49 and 53), and the
operator's ruling of 2026-10-20 on #2777, **"confine first"**; tickets #2777 (SK-14) and #2779
(SK-16). It adds six words to [ADR 0053](0053-an-extension-is-granted-capabilities-one-at-a-time.md)'s
vocabulary and amends [ADR 0041](0041-a-plugin-is-a-subprocess-or-charter-has-no-plugins.md)'s
"watched, not confined" for the extensions that declare them. Each records the change as an
amendment of its own, in [`0041/`](0041/) and [`0053/`](0053/).

**Built by #2777** (the six studio capabilities, the confinement, and purlis's built-ins moved
onto them), #2779 (the Grader and the built-in runner) and #2778 (the SkillRouter).

## Where purlis was

An extension is a program that runs as the operator, one question at a time. purlis is polite
to it but does not hold it: a capability describes what purlis does for an extension, never a
limit on what the extension can do (ADR 0053, and ADR 0041's amendment of 2026-09-22, which
shipped the runtime with no OS sandbox). Write scopes are detected after the fact.

The Skill studio has to be extendable at every seam. People will want sources that are not git,
house lints, their own templates, panels on a skill's tab, grading frameworks such as promptfoo or
Inspect, and retrieval routers. Several of those see things no extension saw before: a Grader
sees every fixture and transcript, and a router sees every prompt of the chats that use it. The
operator ruled that those extensions are held, not just watched.

## The decision

### 1. Six studio capabilities, each with a built-in first user

A **studio capability** is one of six capabilities the Skill studio is built from:

| Studio capability | Manifest word | What purlis does for the extension | Built-in first user |
|---|---|---|---|
| **SkillSource** | `skill-source` | asks it what skills it has, and for one skill's folder at an immutable revision; the review, copy and pin stay purlis's (ADR 0100 §8) | the git source |
| **Grader** | `grader` | asks it to grade a skill, and runs the chats it asks for (§5) | the built-in runner |
| **SkillLint** | `skill-lint` | asks it for findings on a skill's files, shown in the live lint and the install review | the built-in lints (#2771) |
| **SkillTemplate** | `skill-template` | offers its templates as starting points for a new skill, which lands as a draft skill | the blank and starter templates |
| **StudioPanel** | `studio-panel` | asks it for a panel on a skill's tab, drawn with panel blocks (§6) | the Runs and Grades panels |
| **SkillRouter** | `skill-router` | asks it which skills a chat sees, within a deadline (ADR 0102) | the built-in router |

**purlis's own studio features run through the same studio capabilities.** The built-in git
source, lints, templates and panels, the built-in runner and the built-in router are built-in
extensions (or core code behind the same request and answer), so an extension author has every
seam purlis itself uses, and a seam purlis does not use cannot rot.

**No studio capability adds a permission**, and ADR 0053's "what no capability grants" holds:
none hands an extension a secret, a hook, a harness setting, a vault or the machine store. A
SkillSource never writes the plane: it hands purlis a folder, and purlis copies it after the
person's review. A SkillLint never edits a skill, and a SkillTemplate's files become a draft
skill the person accepts.

### 2. An extension that declares a studio capability is confined

**The operator's ruling of 2026-10-20: confine first.** An extension whose manifest names any
studio capability runs its program **confined**, in the same sandbox profile a chat gets
(ADR 0067: Seatbelt on macOS, bubblewrap on Linux, compiled by purlis's core):

- **It reads only its own folder and what purlis hands it**: the files and paths a request
  names, and its own state folder. It does not read the plane, the person's home or another
  extension.
- **It starts programs only from folders it cannot write**: the system's program folders and its
  own folder, less its state folder, its temporary folder and any vault. On macOS Seatbelt holds
  this; on Linux the kernel's Landlock does, and such a program makes no file in memory.
- **It writes only to its own state folder and the paths a request hands it.** Its write scope
  (ADR 0053) is held, not only detected.
- **Its network goes only to the hosts its manifest declares**, each named in the approval
  prompt; none when it declares none. The declaration stops being a claim the extension makes
  about itself (ADR 0041, decision 2's "declared, not granted") and becomes what it can reach.
- **The denial classes hold**, as for every chat: no vault or purlis key is read, and nothing a
  later program runs is written.

**The whole program is confined, for every request**, not only for its studio questions: an
extension that declares `grader` and `events` is one program, and a program is either held or
not.

### 3. No studio capability reaches a model or carries a chat

**A studio capability's declared hosts may never be a model provider's, and may never carry a
chat.** Every model call a studio capability needs goes through purlis running a chat for it
(§5), so the cost is shown, the chat is sandboxed and audited, and the harness and model are the
ones the person chose. purlis enforces this twice:

- **The manifest is refused** when it declares a host that serves a model: one in purlis's list of
  model-provider hosts (those the shipped and declared harnesses and their model sources reach),
  with a sentence naming the host. The person cannot approve such a host for a studio
  capability, because the prompt never shows one.
- **The confined program's proxy refuses those hosts** whatever the manifest says, so a host
  added to the list later is closed to an extension already approved.

### 4. A program purlis cannot confine does not run

**A program purlis cannot confine does not run** (ADR 0031: refuse rather than degrade). On a
machine where the profile cannot be applied, such as Windows today, **an extension that declares
a studio capability does not load at all**: none of its capabilities, studio or not, and a
Notice says why. Loading its other capabilities would run the same program unconfined.

**Third parties can register studio capabilities only once this confinement is in.** Until then
every build refuses a studio capability from an extension that is not built in, with a sentence
naming the word. purlis's built-in studio extensions are trusted through the app's signature
(ADR 0041's amendment of 2026-09-25) and are not affected.

**The approval prompt says what is true.** ADR 0041's amendment of 2026-09-22 requires the
prompt to say that an extension runs with the operator's own access. For a confined extension
that sentence would now be false, so the prompt says the opposite, from the core, pinned by a
test: it runs confined, it reads only its own folder, it reaches only the hosts it names, and it
writes only its own folder. This moves M47's E2 (sandboxed extensions) forward for the studio
capabilities only. Every other extension stays watched, not confined, until E2.

### 5. A Grader asks purlis to run its chats, and never starts a harness

A **Grader** is the extension form of a **grading framework**. It gets the skill and its fixtures
and returns scores. **Every chat a grade needs is a request the Grader sends purlis**, including
the chats in which a model acts as the judge ("judge" here names that role, not a separate
mechanism), and purlis runs it: on a scratch workspace, under the chat sandbox, on the harness
and model the person chose, with the cost shown before anything starts, recorded as an ordinary
run. Confinement and §3 make this the only way: a Grader that started a harness itself would have
no host to reach a model with.

**The fixtures travel in the skill folder**, in skill-creator's shapes (the delegated decision
of 2026-10-20 on #2779):

- `evals/evals.json` holds the outcome evals, byte-compatible with skill-creator. The reader
  accepts both `expectations` and `assertions`, since skill-creator's own docs use both.
- `evals/triggers.json`, the **trigger test file**, a sibling, holds the trigger tests as
  skill-creator's array of a query and whether it should trigger, so skill-creator's own trigger
  runner still reads it.

So the `evals/` folder, not one file, is the exception [ADR 0099](0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md) makes to keeping purlis's data
beside the skill folder. Results never go in the folder: they go to the eval record (MH-15),
tied to the skill's commit, harness and model.

**A trigger is detected from skill runs** (#2766, #2776), which works on every harness, not from
the first tool call, which works only on Claude Code. **The built-in runner follows
skill-creator's method**: each fixture with the skill, without it and against the previous
version, 3 runs each, mean and standard deviation and the difference. A grade starts when the
person asks, or after each edit if they opt in, never on a schedule. Claude Code's
`claude plugin eval` format is a candidate for a second Grader, not for the built-in runner.

**The interface is neutral**, so promptfoo, Inspect or a house framework fits it through an
adapter extension, and the shipped app stays one language. It is four messages, whose fields
and wire spelling are #2779's:

| Message | From | Purpose |
|---|---|---|
| **Grade request** | purlis to the Grader | what to grade: the skill at a commit, its fixtures, the harness and model, and the **arms** to compare. The arms default to three, `with`, `without` and `previous`, the last when the skill has a previous version (decision 43) |
| **Run-chat request** | the Grader to purlis | one chat to run, for an arm or as the judge, with the skills to include and the prompt and files to start it with |
| **Run-chat result** | purlis to the Grader | what that chat did: its run, which opens in one click, its transcript and workspace, the skills it invoked, and its cost |
| **Grade report** | the Grader to purlis | the scores: per fixture, arm and run, a summary per arm with the difference against the baseline, and notes |

A grade is one long question with requests inside it, so the Grader's question has no fixed
deadline of its own while purlis runs its chats, and is stopped when the person cancels the
grade. Each chat it asks for is still held to its turn limit and to the cost the person accepted.

### 6. A StudioPanel draws with panel blocks

A StudioPanel's answer is panel blocks, the same vocabulary a view's answer uses: native, themed
and accessible, with no webview. A panel that needs a block the vocabulary lacks is answered by
adding the block to purlis, for every panel, not by a way out of it. Webview panels wait for
PE-39.

### 7. Experimental until the finish line, then the public API's version 1

While the `skill-studio` flag is off for a machine, the six words are unknown there and refuse a
manifest as any unknown word does. Where it is on, they are marked **experimental**: the
approval prompt and the Extensions list say so, and they may change shape without a protocol
promise. **When the flag flips at the finish line, they become version 1 of the public extension
API (PE-38)**, and from then on change only as ADR 0053 says a capability changes: by a protocol
version, with every earlier version still answered.

## What was rejected

- **Watched, not confined, for studio extensions.** A Grader reads every fixture and transcript
  and a router reads every prompt; an extension doing that as the operator, with the network
  open, is the operator's ruling's reason to confine first.
- **Confining only the studio requests of a program, or loading its other capabilities where it
  cannot be confined.** A program cannot be held for one question and free for the next.
- **A Grader, or any studio capability, that starts its own harness or calls a model directly,
  even with the person's approval of the host.** It would bypass the sandbox, the audit, the
  cost shown first and the harness and model the person chose, so no approval can allow it.
- **One studio capability per grading framework.** The neutral interface lets each framework be
  an adapter, and purlis's own runner proves the interface.
- **Webview panels in v1.** They wait for PE-39's isolation.
- **Publishing the studio capabilities as stable from their first slice.** Their shape is learnt
  by building the studio on them.

## Consequences

- Every studio seam has a built-in user, so the studio capabilities are exercised by the app's
  own tests before any third party relies on them.
- For the first time some extensions are less privileged than purlis. ADR 0028's re-opening
  clause, which ADR 0067 already fired for chats, now applies to these extensions as well.
- Confinement costs a sandbox start per question. ADR 0102 records what that means for a
  router's deadline.
- Third-party studio extensions wait for the confinement, and on Windows for a Windows sandbox.
- purlis keeps a list of model-provider hosts, and it grows with every harness and model source
  purlis learns.
- No studio capability hands an extension a credential in v1, so a non-git source that needs one
  waits. A private git source signs in through purlis (ADR 0100 §5).
- `CONTEXT.md` gains the six studio capabilities, and its **Extension** and **Capability** say
  which extensions are confined.

## Amendments

The amendments to this decision are files of their own in [`0101/`](0101/), one per
amendment, named `<yyyy-mm-dd>-<slug>.md`. A new amendment is a new file there and never a
section added to this record, so two pull requests that amend it do not touch the same lines
(#1982).
