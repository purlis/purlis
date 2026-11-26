# Prerequisites: what a project needs from the person before its chats can work

**Accepted 2026-11-26**: the person's decisions on the prerequisite design, grilled for M75
(milestone 55) after #7714, and revised the same day after a three-reviewer round. It amends
[ADR 0121](0121-attention-what-waits-on-you-reaches-you-and-nothing-else-interrupts.md)
([2026-11-26](0121/2026-11-26-an-open-prerequisite-may-interrupt-with-a-modal.md)),
[ADR 0104](0104-the-window-is-built-from-component-families-on-react-aria-each-primitive-wrapped-once.md)
([2026-11-26](0104/2026-11-26-the-dialog-family-serves-an-app-raised-alert.md)) and
[ADR 0120](0120-a-purlis-command-exits-70-for-an-internal-failure-and-only-then.md)
([2026-11-26](0120/2026-11-26-a-start-held-by-a-prerequisite-exits-75.md)) and
[ADR 0125](0125-blocks-are-off-by-default-and-opt-in-per-family.md)
([2026-11-26](0125/2026-11-26-a-pressed-accept-reaches-a-running-chat-s-guards.md)), and replaces
`ProjectPolicyNotice` for two of its states under
[ADR 0112](0112-one-notice-for-one-need-a-person-may-allow-an-undeclared-program.md)'s one notice for
one need. ADR 0125 is amended for one case alone, a running chat's guards after Accept (§5); its
other rules on who may loosen a project's policy, and what Accept accepts, stand.

## Where purlis was

A project's policy acceptance (ADR 0125 §3) has three readings on this machine
(`purlis_core::sandbox::local::PolicyAccepted`): **Never** (nothing kept, the shipped `watch`
stands), **Held**, and **Broken** (a record was kept and what is here is not it). Beside them the
policy may be **Waiting**: the committed file is looser than what this machine accepted.

- **Broken stops nearly everything and says almost nothing.** Every family is enforced at the full
  sandbox and no chat dispatches to another persona. The only sign is a Notice among the others
  (`app/src/ProjectPolicyNotice.tsx`) and the pane head's `strict` (`app/src/PolicyMark.tsx`).
  After an app update that macOS treats as a different program, every project reads Broken, and
  Accept cannot recover it (#7714). Engineers who updated purlis hit refusals with no
  explanation.
- **Refusals blame the wrong thing.** A dispatch refused because the acceptance is Broken says
  "purlis could not read the list of pairs you said never to" (`dispatchnever::unread_said`), and
  a guard's refusal under Broken or Waiting names its family and tells the person to set it to
  `log` (`toolgate::OVERRIDE_NOTE`), never that the policy is not accepted here.
- **The window decides when to look.** It reads `project_policy_state` on focus and on a change of
  the settings file, so the CLI and `purlis doctor` see nothing of it, and another window learns
  of an Accept only when it is next focused.

## Terms

- **Strict mode**: every family enforced at the full sandbox, with no cross-persona dispatch. It is
  what a project runs under **Broken**, and only there. It is not the `strict` template, which
  leaves the sandbox to `[sandbox]` and denies no dispatch.
- **The policy in force** (`enforcement::in_force`): what a chat runs under. Under Broken it is
  strict mode. Under **Waiting** it is the stricter of the committed file and what this machine
  accepted before, so nothing is tightened beyond what was accepted and nothing waiting is
  loosened.
- **The project's tab in the title bar**, and its **dot**: the place and the mark that show a
  project not in front has an open prerequisite (§3).
- **After an app update**: the first open of a project after purlis relaunched as a new build.

## The decision

### 1. A prerequisite is the core's

A **prerequisite** is a condition a project must meet before its chats can work, which only the
person can satisfy, in the window. `purlis-core` defines each one and decides when it is open; the
window never decides one itself, and stops polling `project_policy_state` on focus for it.

The mechanism is generic. It ships with **one prerequisite, the policy one**; a sign-in that
expired and a migration are later prerequisites, each with a decision of its own.

### 2. The list and its event

- **The app's process holds the list**, per project, computed by `purlis-core`. The CLI and
  `purlis doctor` read it from the app over the hook socket, and say so when the app does not
  answer ("purlis's window is not running, so purlis cannot say what this project needs"); they
  never compute it themselves.
- **One "prerequisites changed" event per project carries the whole ordered list**, never a delta.
  The core sets the order. Each entry has:
  - its **kind** (today only `policy`);
  - its **state**: `open`, or `deferred` once answered for this run (§6);
  - its **reason**: `broken`, `waiting`, or both;
  - the policy **digest** an Accept must name;
  - the **waiting rows**, as `PolicyState` (`enforcement::standing`);
  - **needs a new key**: whether Accept must first replace a foreign seal key (§4);
  - **after a new key**: whether the record broke because purlis replaced its key (§4).
- **The event is pushed after the new list is kept**, so a reader that asks then gets the same list.
  Any reader can ask for the current list at any time; a window that opens, or a reader that missed
  an event, asks.
- **The list is evaluated again** on a project's open, on a move of the plane root's `HEAD`, on a
  change to the settings file, the local settings file or `/etc/purlis/policy.json`, on a change to
  the acceptance record or the seal key, and after every answer.
- **The window trusts none of the event's text blindly.** It draws the summary and the digest it
  presses from one entry, one read, never a summary from one read and a digest from another, and it
  draws free text inert (§4).

### 3. The policy prerequisite: Broken and Waiting

- It is open while this machine's acceptance of the project's policy is **Broken** or **Waiting**.
- **Never alone is no prerequisite**: the shipped `watch` default stays the one-time Notice
  `PolicyWatchAnnounced`, unchanged. But nothing kept plus a committed policy looser than `watch`
  counts as **Waiting**.
- **Broken and Waiting together make one prerequisite**, and one modal. **Under Broken the reason is
  `broken` alone**; `waiting` joins it only where the committed policy is looser than the shipped
  `watch`, the same rule as for Never, with no read of the unsealed record.
- **When it is raised:** on a project's open, on a policy change mid-session, and after an app
  update. Where the project is not the one in front, a **dot** shows on the project's tab in the
  title bar, and the modal opens when the person switches to it. The dot shows only while the
  prerequisite is open and unanswered this run, and its accessible name is the core's words for it.

### 4. The modal

- **An alert dialog of the dialog family** (ADR 0104, as amended 2026-11-26), one at a time; several
  open prerequisites are shown one after another in the core's order, each saying where it stands
  ("1 of 2").
- **Title:** "<project>'s policy needs your answer".
- **The why-line says the effect of each answer plainly, by state:**
  - Under Broken: purlis cannot vouch for what was accepted on this machine before, so until the
    person answers, the project's chats work in strict mode.
  - Under Waiting: the committed policy loosens what this machine accepted; until the person accepts
    it, "your chats keep the policy you accepted before". Where nothing was kept, "your chats keep
    purlis's shipped default (watch) until you accept".
- **The summary:**
  - Under **Waiting**: what Accept would loosen, as `enforcement::standing` gives it, listed first,
    never collapsed, with its count. An administrator's locks read "stay enforced whatever you
    accept". What does not run at this level stays said (`notInForceSaid`). Tightenings are not
    listed: they are already in force.
  - Under **Broken**: computed against strict mode, never by reading the unsealed record, under the
    words "purlis cannot vouch for what you accepted before, so this is everything Accept puts in
    force".
  - Each row gets one plain clause of consequence (for example, "Bash: commands it would refuse run,
    and are logged").
  - **Free text from the file** (branch names, persona names, the project's name) is drawn inert, in
    code font, cut to a cap, with control and bidirectional characters stripped.
  - A link opens the full policy view (Settings › Project › Policy). It closes the modal unanswered:
    the prerequisite stays open, with its Notice and dot.
- **Answers:**
  - The answer that grants nothing, in the family's `cancel` slot: **Work in strict mode** under
    Broken, **Keep my current rules** under Waiting (and under both, Work in strict mode). It leads,
    is Esc's, and has focus first.
  - **Accept**, in the `main` slot, of the `grants` variant, never pressed by Return. Where the seal
    key is foreign, whatever made it, Accept reads **Make a new key and accept** (#7714); the words
    never say where the key came from.
- **Accept keeps today's rule:** it accepts exactly what was shown, bound to its digest; if the
  committed file, the local file or a lock changed since, nothing is accepted (`docs/policy.md`).
  The press binds to the plane the modal was raised for, whichever project is in front when it
  lands. Accept in Settings › Project › Policy is the same call.
- **When the Keychain refuses** a new key (Deny, a wrong password, a locked Keychain), nothing is
  accepted. The modal stays and says "macOS did not let purlis replace its key, so nothing was
  accepted", and the press can be made again. The digest is checked after the macOS prompt, not
  before it.
- **After a new key**, every other project that was accepted on this machine stays **Broken**: its
  record was sealed with the old key. Each raises its own prerequisite, whose entry says **after a
  new key** (§2) and whose why-line says "purlis made a new key on this machine; accept this
  project's policy again". None reads Never. A record broken for any other reason never gets these
  words.
- **After Accept**, a Notice of the `news` tone: "Policy accepted for <project>. Guards follow it
  now; a chat started in strict mode keeps the full sandbox until you restart it." Nothing else
  changes.

### 5. What an open prerequisite holds

- **A chat the person starts in that project, from the window or the CLI, waits** until they answer.
  So does a pool or runner chat the person starts: it waits as a local one does. It is not a
  **Hold**: nothing is turned into an ask. A CLI start's wait is in §8.
- **Running chats keep running, under the policy in force**: strict mode under Broken, the policy
  accepted before under Waiting.
- **Once the person presses Accept, a running chat's guards follow the accepted policy from its
  next step**, and so do a task it dispatches and its forge route's `guards.merge`. The hook asks
  the app, which knows the press, so `PURLIS_POLICY`, the start's resolution, the chain's recorded
  policy and the route's start no longer floor a loosening the person pressed (ADR 0125, as
  amended 2026-11-26). The chat's sandbox level stays as it started. Only the press lifts the
  floor: where the app does not answer, or its answer does not say the person pressed Accept
  since the chat started, the floor still holds.
- **The press updates the chain's record** (R10): the app clears the inherited chain policy of
  each running chat in this project the press lifts, in its own record, which no chat can write.
  That is the only thing the press writes. A chain is lifted only where each chat above it is
  found, by id, among this project's open chats; one that has closed or is not found keeps the
  floor (fail closed), and so does a chain through another project.
- **The start is recorded** (R11): each running chat's start policy, as `PURLIS_POLICY` spells
  it, is kept in the app's session record (a `policy` on the host record's `Started`), written
  when the chat starts and never by the press.
- **After an app restart**, the fact of the press is gone (it is kept in memory), so every chat
  still running has its start's floor back until it restarts. The re-attached forge route and the
  chat's Notice read that floor from the recorded start, and the Notice says so. A dispatched chat
  the press lifted gets its start's floor back as any chat does, and that floor holds the chain's
  policy it started with (`PURLIS_POLICY` folds it in). Its record stays cleared, so its restart
  starts under the accepted policy.
- **These start at once, under the policy in force, and do not wait:** a chat restored at open, a
  task a running chat dispatches, a chat a running chat opens with `purlis handoff` (it starts
  under the policy in force, strict mode under Broken, as a dispatched task does), and a scheduled
  or loop start.
- **The core owns the hold.** The window only shows it and releases it, through an answer.

### 6. The answer that grants nothing

- **Its lifetime.** It is kept in the app's memory, per project, against the digest and reason it
  was shown for, and never written to disk. It ends on a restart, a change of digest, or a change
  of reason. A change back to a digest already answered this run does not open the modal again.
- Meanwhile the prerequisite's entry reads `deferred`, a held start is released under the policy in
  force, and a new chat starts under it.
- The prerequisite's **Notice** stands, and cannot be dismissed while the prerequisite is open; its
  main action opens the modal again. That Notice **replaces `ProjectPolicyNotice`** for Broken and
  Waiting (one notice for one need, ADR 0112). The pane mark is unchanged: `strict` under Broken,
  the template's name under Waiting.

### 7. Refusals name the cause

Every guard refusal that an open prerequisite causes says so, for example: "…refused because this
project's policy isn't accepted on this machine — accept it in purlis's window". Such a refusal
never tells the person to set the family to `log`. The dispatch refusal that blames "the list of
pairs you said never to" for a Broken acceptance names the policy instead; a nevers file that does
not read for its own reason keeps today's sentence.

### 8. Not an ask, and only the window accepts

- **A prerequisite is never in Asks** and adds nothing to the ✋ count or the Dock badge. It sends
  no system notification. Its signals are the modal, the dot on the project's tab, its Notice, and
  the CLI's and `purlis doctor`'s words.
- **Accept is purlis's own window's alone; the answer that grants nothing is given there or on a
  linked page.** No CLI command, hook-socket call or chat tool gives either.
- **A linked page** (one not in purlis's own window) shows the modal with the answer that grants
  nothing and, in Accept's place, "Accept it from purlis's own window on this machine".
- **Answering in one window closes it in every window**, through the same event.
- **`purlis doctor` and the CLI show the open prerequisites.** Doctor's finding is the
  prerequisite's own Notice line, not a second one.
- **A CLI start that waits** prints what is open and that it is answered in purlis's window on this
  machine. With a terminal it waits up to 90 seconds, and the chat starts when the person answers.
  **A start with no terminal, or whose wait runs out, exits 75** (`EX_TEMPFAIL`) with the reason
  (ADR 0120, as amended 2026-11-26); **Ctrl-C ends it as any interrupted command.**
- **Acceptance stays per machine.** A pool or runner chat keeps the acceptance of the machine it was
  started from, and its refusals name the prerequisite and that machine's window.

### 9. Built in this order

1. **Quick relief:** one press for a new key (#7714), and refusals that name the cause (§7).
2. **Then:** the core's list and its event with the policy prerequisite; holding starts and the
   answer's lifetime; the modal and the Notice; the dot; the CLI and doctor; runners; the docs.

## Consequences

- ADR 0121's "nothing else interrupts" has one exception, an open prerequisite's modal, and its
  decisions 2 and 16 name it (its amendment of 2026-11-26).
- The dialog family serves an alert the app raises with no press (ADR 0104's amendment of
  2026-11-26).
- A running chat's guards, its chain and its forge route follow a pressed Accept from its next
  step, its sandbox level as it started (ADR 0125's amendment of 2026-11-26), so the Notice after
  Accept says what is true. The press clears the chain's record of each chat it lifts, the only
  thing it writes (`app/reopen.json`, Clone state).
- Each running chat's start policy is recorded on the host record's `Started` (R11; a new row
  in `docs/plane-format.md`, Machine, device-bound, transient), so after an app restart the
  re-attached forge route and the Notice read the floor the chat started under.
- ADR 0120 documents a second code beside 70: 75 for a start a prerequisite held (its amendment of
  2026-11-26).
- `ProjectPolicyNotice` keeps only what is not a prerequisite: the policy purlis could not read
  (`policy-unreadable`). `PolicyWatchAnnounced` is unchanged.
- The window's focus polling of the policy state goes; the window, the CLI and doctor all read the
  app's one list, so they cannot disagree.
- A new chat may wait at its start for a reason that is not an ask, so every start path that waits
  (window, CLI) has to say why, and every one that does not (restore, dispatch, handoff by a chat,
  schedule, loop) has to start under the policy in force; a pool or runner chat the person starts
  waits as a local one does.
- A stray key while the modal opens can only choose the answer that grants nothing: focus starts
  there, Accept is never found by Return, and the settle guard holds every press for a moment.
- Not chosen: a ✋ count or a system notification for a prerequisite (it stops no running chat, and
  Asks would hold a question only the window can answer); a CLI accept (a chat could run it); the
  window deciding when a prerequisite is open (each surface would answer differently); a modal for
  **Never** alone (the shipped default is not a loosening the person must weigh); keeping the
  answer that grants nothing on disk (a restart is when the person should be asked again).
