# Amended 2026-09-25: events, and a quoted section in the session-start briefing (purlis#343)

The `events` and `briefing` capabilities (ADR 0053, protocol 2) are the first that start an
extension's program **without the operator opening anything**: after a core action it hears
about, and when a chat starts. Decision 1 is unchanged — one process per question, the gate
re-taken before each, the same bounds and kill — and decision 2's table is unchanged: neither
capability reaches the machine store, `reopen.json`, harness profiles, vaults or a secret.
What it adds to the threat model:

- **"Never on its own" is no longer true of such an extension, and the prompt says so.** The
  program line of the approval prompt names when purlis starts it — after each event it
  hears, when a chat starts — instead of the view-only sentence, and every event it hears is
  listed. The lists are inside the manifest's bytes, so adding an event re-asks.
- **An event never changes the action it reports.** It is delivered after the action has
  finished, by the surface that did it. **In the app it is off the command's path**: the app
  starts a thread once a command — or auto-save — has its answer, and returns the answer
  without waiting. **The `purlis` binary answers first and then waits**: it prints and flushes
  its answer, with its exit status already decided, and only then asks, before the process
  ends. A shell or a chat waiting for the process to end therefore waits up to one deadline,
  and only when an extension that hears that event is slow — every extension is asked at once
  — which is the cost of keeping the note where the person who ran the command sees it. A
  process detached from the command would cost nothing and could tell nobody that an extension
  missed the event. Each extension is asked on a thread of its own with the normal deadline,
  and an event waits its turn behind a question still in flight rather than being refused. A
  failure or a timeout is one note naming the extension — the status line's extension notes in
  the app, a line on stderr in a terminal — and nothing else.
- **Both are protocol-2 questions like #341's actions.** Each carries `writes`, and purlis
  watches the plane while it is answered; a write outside the declared paths is one more note
  naming the extension. Protocol 2 was unreleased when they were added, so they grew it rather
  than bumping to 3.
- **A chat starting is the one event on a shorter clock.** `session-started` is told inside
  the session start's bounded wait below, beside the briefing question and with its bounds,
  because the hook that reports it is what holds the chat's start: told after it, it would be
  told by a process the harness is still waiting on.
- **A briefing section is extension-written text in front of the model, so it is quoted as
  data.** It sits under a line of purlis's own naming the extension and saying it is data,
  not instructions, and that nothing in it is a task, a permission, a hook or a setting; every
  line of it is set off with `> `. It is cut at 1,500 characters, all sections together at
  6,000, and a section holding anything `panel::undrawable` refuses is left out whole, with a
  line of purlis's own saying it is missing. It cannot add a permission, a hook or a setting
  by construction: it is one string inside `additionalContext`, which purlis serializes
  itself beside its own `hookEventName`.
- **A chat's start is bounded whatever the extensions do.** `charter hook sessionstart` asks
  every extension that briefs or hears the start at once, gives each question 2 seconds and
  the whole of it 3, then kills every program still running and briefs the chat without it. A
  machine with no such extension pays one read of the record, and the briefing is byte for
  byte what it was before.
- **A changed or turned-off extension adds nothing and hears nothing.** The record, the
  project and workspace on/off (ADR 0048) and the fingerprint are asked before anything
  starts; an extension that changed since it was approved is a note for the operator and adds
  not even the missing-section line.
- **A built-in extension (purlis#339) hears and briefs like any other, where purlis knows
  it is one.** The app's executor knows its bundle, so a built-in hears the events the window's
  commands and auto-save report, and one turned off on this machine hears nothing. The
  `purlis` binary passes no bundle — as `charter statusline` does — so a built-in neither
  hears a terminal's commands nor briefs a chat's start. That is deliberate for now: the binary
  cannot tell an app bundle from a folder claiming to be one, and a briefing that trusted a
  path the binary guessed would be trusting a file's word.
- **A fork carries an extension's workspace folder even while it is off.** It is data the
  extension keeps in the plane, and a fork is not the moment to decide it stays behind. The
  folder is one plain name declared in the fingerprinted manifest, never one of purlis's own
  names in a workspace, only for an approved and unchanged extension, and never a clone.
- **What it does not close.** The program still runs as the operator (`RUNS_AS_YOU`). An
  extension that hears events runs more often than one the operator opens, which is more
  chances to do what any program running as the operator can do; the prompt says when, and
  that is the whole of the answer.
