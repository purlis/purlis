# The Skill studio is built from six extension capabilities, and an extension that declares one is confined

**Accepted 2026-10-20**, from the operator's spec #2762 (M65, Skill studio), whose decisions were
grilled with the operator on 2026-10-20 (decisions 8, 22, 27, 45, 48, 49 and 53), and the
operator's ruling of 2026-10-20 on #2777, **"confine first"**; tickets #2777 (SK-14) and #2779
(SK-16). It adds six words to [ADR 0053](0053-an-extension-is-granted-capabilities-one-at-a-time.md)'s
vocabulary and amends [ADR 0041](0041-a-plugin-is-a-subprocess-or-charter-has-no-plugins.md)'s
"watched, not confined" for the extensions that declare them. Each records the change as an
amendment of its own, in [`0041/`](0041/) and [`0053/`](0053/).

**Built by #2777** (the six capabilities, the confinement, and purlis's built-ins moved onto
them), #2779 (the Grader and the built-in runner) and #2778 (the SkillRouter).

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

### 1. Six capabilities, each with a built-in first user

| Capability | Manifest word | What purlis does for the extension | Built-in first user |
|---|---|---|---|
| **SkillSource** | `skill-source` | asks it what skills it has, and for one skill's folder at an immutable revision; the review, copy and pin stay purlis's (ADR 0100 §8) | the git source |
| **Grader** | `grader` | asks it to grade a skill, and runs the chats it asks for (§3) | the built-in runner |
| **SkillLint** | `skill-lint` | asks it for findings on a skill's files, shown in the live lint and the install review | the built-in lints (#2771) |
| **SkillTemplate** | `skill-template` | offers its templates as starting points for a new skill, which lands as a draft skill | the blank and starter templates |
| **StudioPanel** | `studio-panel` | asks it for a panel on a skill's tab, drawn with panel blocks (§4) | the Runs and Grades panels |
| **SkillRouter** | `skill-router` | asks it which skills a chat sees, within a deadline (ADR 0102) | the built-in router |

**purlis's own studio features run through the same capabilities.** The built-in git source,
lints, templates and panels, the built-in runner and the built-in router are built-in
extensions (or core code behind the same request and answer), so an extension author has every
seam purlis itself uses, and a seam purlis does not use cannot rot.

**No capability adds a permission**, and ADR 0053's "what no capability grants" holds: none
hands an extension a secret, a hook, a harness setting, a vault or the machine store. A
SkillSource never writes the plane: it hands purlis a folder, and purlis copies it after the
person's review. A SkillLint never edits a skill, and a SkillTemplate's files become a draft
skill the person accepts.

### 2. An extension that declares a studio capability is confined

**The operator's ruling of 2026-10-20: confine first.** An extension whose manifest names any of
the six words runs its program **confined**, in the same sandbox profile a chat gets (ADR 0067:
Seatbelt on macOS, bubblewrap on Linux, compiled by purlis's core):

- **Network only to the hosts its manifest declares**, each named in the approval prompt; none
  when it declares none. The declaration stops being a claim the extension makes about itself
  (ADR 0041, decision 2's "declared, not granted") and becomes what it can reach.
- **Writes only to its own state folder and the paths a request hands it.** Its write scope
  (ADR 0053) is held, not only detected.
- **The denial classes hold**, as for every chat: no vault or purlis key is read, and nothing a
  later program runs is written.

**The whole program is confined, for every request**, not only for its studio questions: an
extension that declares `grader` and `events` is one program, and a program is either held or
not. **A program purlis cannot confine does not run** (ADR 0031: refuse rather than degrade).
On a machine where the profile cannot be applied, such as Windows today, the extension's
studio capabilities are not loaded and a Notice says why.

**Third parties can register studio capabilities only once this confinement is in.** Until
then a release build refuses the six words from an extension that is not built in, with a
sentence naming the word. purlis's built-in studio extensions are trusted through the app's
signature (ADR 0041's amendment of 2026-09-25) and are not affected.

**The approval prompt says what is true.** ADR 0041's amendment of 2026-09-22 requires the
prompt to say that an extension runs with the operator's own access. For a confined extension
that sentence would now be false, so the prompt says the opposite, from the core, pinned by a
test: it runs confined, it reaches only the hosts it names, and it writes only its own folder.
This moves M47's E2 (sandboxed extensions) forward for the studio capabilities only. Every other
extension stays watched, not confined, until E2.

### 3. A Grader asks purlis to run its chats, and never starts a harness

A **grading framework** arrives as a Grader. It gets the skill and its fixtures and returns
scores. **Every chat a grade needs, the judge's included, is a request the Grader sends
purlis**, and purlis runs it: on a scratch workspace, under the chat sandbox, on the harness and
model the person chose, with the cost shown before anything starts, recorded as an ordinary run.
Confinement makes this the only way: a Grader that started a harness itself would have no
network to reach a model with, unless the person approved that host by name.

**The fixtures travel in the skill folder**, in skill-creator's shapes (the delegated decision
of 2026-10-20 on #2779):

- `evals/evals.json` holds the outcome evals, byte-compatible with skill-creator. The reader
  accepts both `expectations` and `assertions`, since skill-creator's own docs use both.
- `evals/triggers.json`, a sibling, holds the trigger tests as skill-creator's array of a query
  and whether it should trigger, so skill-creator's own trigger runner still reads it.

So the `evals/` folder, not one file, is the exception ADR 0099 makes to keeping purlis's data
beside the skill folder. Results never go in the folder: they go to the eval record (MH-15),
tied to the skill's commit, harness and model.

**A trigger is detected from skill runs** (#2766, #2776), which works on every harness, not from
the first tool call, which works only on Claude Code. **The built-in runner follows
skill-creator's method**: each fixture with and without the skill (or against the previous
version), 3 runs each, mean and standard deviation and the difference. A grade starts when the
person asks, or after each edit if they opt in, never on a schedule. Claude Code's
`claude plugin eval` format is a candidate for a second Grader extension, not for the built-in
runner.

**The interface is neutral**, so promptfoo, Inspect or a house framework fits it through an
adapter extension, and the shipped app stays one language. It has four messages. Their exact
spelling on the wire is #2779's; their content is this:

| Message | From | Holds |
|---|---|---|
| **Grade request** | purlis to the Grader | the skill (name, folder, commit, and the previous commit when one is compared); the fixtures (outcome evals and trigger tests, as read from `evals/`); the harness; the agent model and, separately, the judge model; the **arms** to compare, `with`, `without` and `previous` (skill-creator's with-skill, without-skill and old-skill), `with` and `without` by default; runs per fixture, 3 by default; the Grader's own options |
| **Run-chat request** | the Grader to purlis | the arm, or `judge`; the skill set to include or exclude, and a skill revision; the prompt (a fixture's, or a judge's carrying a transcript); files to stage into the scratch workspace; a turn limit; an output schema for a structured judge answer; and, for a judge, a prior run whose workspace it reads without writing |
| **Run-chat result** | purlis to the Grader | the run's id, which opens its chat in one click; the final text; the transcript, normalised to messages and tool calls; the workspace it left; the skills invoked and at which turn; the first tool call; tokens, cost and duration; an error, if it failed |
| **Grade report** | the Grader to purlis | the Grader's id and version; a score per fixture, arm and run (pass, a score from 0 to 1, each check with whether it passed and its evidence, a trigger rate for trigger tests, the run's id, tokens and duration); a summary per arm (mean, standard deviation, minimum and maximum of pass rate, seconds and tokens), the difference against the baseline arm, and trigger counts (passed, total, false positives, false negatives); notes; and suggested fixes to the fixtures |

A grade is one long question with requests inside it, so the Grader's question has no fixed
deadline of its own while purlis runs its chats, and is stopped when the person cancels the
grade. Each chat it asks for is still held to the turn limit and the cost the person accepted.

### 4. A StudioPanel draws with panel blocks

A StudioPanel's answer is panel blocks, the same vocabulary a view's answer uses: native, themed
and accessible, with no webview. A panel that needs a block the vocabulary lacks is answered by
adding the block to purlis, for every panel, not by a way out of it. Webview panels wait for
PE-39.

### 5. Experimental until the finish line, then the public API's version 1

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
- **Confining only the studio requests of a program.** A program cannot be held for one
  question and free for the next.
- **A Grader that starts its own harness or calls a model directly.** It would bypass the
  sandbox, the audit, the cost shown first and the harness the person chose.
- **One capability per grading framework.** The neutral interface lets each framework be an
  adapter, and purlis's own runner proves the interface.
- **Webview panels in v1.** They wait for PE-39's isolation.
- **Publishing the capabilities as stable from their first slice.** Their shape is learnt by
  building the studio on them.

## Consequences

- Every studio seam has a built-in user, so the capabilities are exercised by the app's own
  tests before any third party relies on them.
- For the first time some extensions are less privileged than purlis. ADR 0028's re-opening
  clause, which ADR 0067 already fired for chats, now applies to these extensions as well.
- Confinement costs a sandbox start per question. A SkillRouter answers within a deadline
  (ADR 0102), so that cost is measured as part of #2778.
- Third-party studio extensions wait for the confinement, and non-Windows only until a
  Windows sandbox exists.
- A private source or panel that needs a credential has none to use in v1: no capability hands
  an extension a secret. A private git source signs in through purlis (ADR 0100 §5).
- `CONTEXT.md`'s **Extension** says which extensions are confined.
