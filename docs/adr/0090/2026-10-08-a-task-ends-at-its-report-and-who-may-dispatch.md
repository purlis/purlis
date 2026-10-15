# Amended (2026-10-08): a task ends at its report, and who may dispatch to whom

The operator ruled on these on 2026-10-08, in the grill that is spec #1483 (rulings V100-1 to
V100-76). This is the one amendment of that day: each ticket recorded its part as it was
built, and the trains that carried them joined the parts here. Items 1 and 2 came with train
62 (#1485), the section on ending a task by hand with train 63 (#1488), and items 3 to 18
with train 64 (#1502 to #1507, #1509, #1521). **Where two items once said
different things, what stands here is the later ruling**, and the item says so. Where this
section and the changes above differ, this section holds; everything else in this record
stands.

**1. A task ends at its report (V100-1, V100-8, V100-9, V100-10).** #1485 builds it. This
replaces one line of *Amended and accepted (2026-10-07)*: "A persona chat stays open after it
reports, marked as reported, until the person or its asking chat closes it."

- **A task's program ends when it reports.** The report is delivered to the asking chat first,
  and purlis then ends the program, under the lock that already orders a report against an
  exit. Its row stays under the asking chat as a finished entry, with its outcome and its
  report, read from the dispatch record.
- Done and cancelled rows fold into one "Finished (n)" line with Clear finished. Failed, ended
  without a report and closed by the person stay rows of their own until cleared.
- Finished rows last until the asking chat closes or they are cleared, and are there after the
  app is restarted. Clearing removes rows only: the dispatch records stay.
- **Reopen** resumes a finished task's conversation as an ordinary chat with a tab. It is no
  longer a task: it sends no second report, and the asking chat is not told.


A handoff's chat is not a task and is not ended by its report. **A finished task reopened is
started on the profile its dispatch chose, and is held to item 16 as that dispatch was.**

**2. Rulings added in review of item 1** (delegated to the dispatcher; the operator is told).

- **A task that is working again is never ended mid-turn.** A key of the person's in its pane
  after the report, or a Smart close of it beginning, stands the end down for good. The bound
  on a turn that does not end is the reporting turn's alone.
- **A chat the person is looking at is not ended under them.** While the task is the chat in
  front, the end is held, and it is carried out when they move away from it. No Notice.
- **A persona chat the person started with Ask from a tab is never ended by purlis at its
  report.** It is the person's conversation: it stays open, and its finished row appears when
  they close it.
- **`blocked` does not end the program.** A blocked task stays open as its own row: it is
  waiting on something, and its conversation is what the next step needs.
- **A task kept open reports once more** (2026-10-11, #1768; delegated; the operator is told).
  A task whose report came out `blocked`, and a task the person took up again after its report
  (a key of theirs in its pane), may send one more report. It reaches the chat that asked as any
  report does, and wakes it the same way, or is kept for the workspace when that chat has gone.
  Its dispatch record ends with it and keeps the earlier report, so the timeline shows both,
  the later one last. Nothing waits on that report: the chat that asked does not, and purlis
  says nothing in its place if the task ends without it. A report that comes out `blocked`
  again leaves one more again, and so does each time the person takes it up. `done`, `failed`
  and `cancelled` still settle the task. A task the person's key took up again, blocked or
  not, is not ended at its later report: that stand-down is for good. Its record says
  `kept_open`. The later report takes the blocked report's needs-you item on the chat that
  asked, and where that chat had gone, it is the report a reopened asking chat is handed.
  Until that one more report it may also send the chat that asked progress notes and
  questions, as a task that still owes its report does (2026-10-12, #1786): the same limit on
  messages a minute between the pair, the same Activity lines, and its question may be
  answered by that chat or by the person. After it, they are refused as before. Unchanged: a
  handoff owes no report, purlis's `ended without a report` is final, a stop takes a report
  only from a task that still owed one, and a finished task reopened reports nothing.
- **A task the person stopped does not fold into Finished (n)**, whatever its own last report
  says. It stays a row of its own.
- **A task that had reported when the app quit is not started again.** At the next launch it
  is a finished row, read from its record.


## Ending a task by hand (2026-10-08, V100-5, V100-6, V100-7, V100-18; #1488)

- **Two ways, and the asking chat is told which.** Stop and get its report, and Close now, go
  through the one stop there is. The asking chat reads one of three ends in purlis's words:
  stopped by the person (with the task's short report, quoted as data), closed by the person
  (no report), ended without a report. The first two add: "The person ended this task. Do not
  dispatch it again unless they ask."
- **The stop is the outcome from the moment it is recorded**, under the lock a report is taken
  under. A report sent after that is the stop's report, whatever it says of itself. One that
  landed before it was an ordinary report, and the task ended by itself.
- **Nothing is typed into a task purlis may not type into.** There only Close now is offered,
  and a stop pressed all the same closes the task as it stands. A stop is bounded and never
  leaves a task stopping.
- **A task is not stopped as a chat is.** Its row has its own two rows and no Stop chat rows;
  a session and a handoff's chat are unchanged. The record keeps who ended a task
  (`ended_by`) and which way (`ended_way`).
- **Only the window asks for either.** No hook line and no chat's command reaches them.

### Rulings added in review of #1488 (2026-10-08, delegated to the dispatcher; the operator is told)

- **Ending a task always takes a second step, and never a modal dialog for an idle task.**
  Delete on a row, the breadcrumb's two controls and the menu's rows ask in place ("Stop it" or
  "Close it", and "Keep", which has the keyboard). The one modal question is for a task in the
  middle of a turn or with tasks of its own still working. This narrows V100-18's "no dialog
  for an idle task" to "no modal dialog".
- **A close ends no task.** The cross and the close shortcut on a task's own tab send the task
  back to the Chats list, and it keeps working. A close of several tabs does the same for the
  tasks among them, and says so in one line.
- **A chat is in its stop until its program is gone.** Its word to the chat that asked and
  its close are one step under the lock a report is taken under. A report in flight for a task
  the person closed is the stop's report, never an ordinary one, and no task of a stop starts
  a chat.
- **Every command that ends, starts or restarts a chat on the person's word is the
  window's alone**, over its own channel to the app, and none is served on a link: ending a
  task and the question it asks first, stopping a chat or every chat, closing a chat or a
  project, starting a chat or a shell as the person, and starting one again. The rule is what
  a command does, it fails closed, and the list is `ui::WINDOW_ONLY`. The other commands that
  act as the person (an answer to a question other than a chat's ask, typing into a chat,
  saying what is on screen) are not under this rule: they are served to a local window of the
  same build, and whether they join the list awaits a ruling.
- **What "a task cannot forge them" means.** A task cannot produce any of the three ends or
  the fixed sentence through the report channel. The folder reports wait in is writable from
  outside a sandbox, and that exposure stays with issue 1457.
- **Counts.** A task the person stopped or closed is done with, not failed: it is counted with
  done and cancelled. Failed is failed, blocked, ended without a report and did not start. The
  rows of tasks the person ended still never fold.
- **On a harness purlis does not type into, Stop is drawn disabled with its reason**, and
  Close now is the offer.

**3. A grant is one-way (V100-22).** A grant from one persona to another allows nothing the
other way. A report back needs none. A new task the other way is its own pair.


**4. A grant may be for any persona (V100-23).** Change 3 says a grant "names the pair". It may
now also name only who dispatches: *chats running as this persona may dispatch to any persona*.

- It is made on purpose, in Settings, at two of the three levels: me on this machine, and
  everyone in this project (`"*"` in the persona's list of `[dispatch.grants]`). Never for one
  chat.
- **The Notice never offers it, and no answer to a Notice makes one.** The first dispatch of a
  pair still offers that pair at the three levels.
- It covers a persona added later.
- A teammate's, arriving by a pull, covers nothing on a machine until someone there allows it
  in Settings. The Notice that says it arrived tells of it and may decline it, and never
  accepts it (item 14). The acceptance is kept apart from the acknowledgement of pairs, and is
  dropped as a pair's is: where a commit took the grant out, never where the file on disk
  merely lacks it (item 14).
- It may be limited to one workspace, as a pair may (item 13).
- Policy locks hold under it, and so does the loop rule.
- **For an unattended chat it counts in that chat's own workspace only.** Such a chat starts a
  chat as another persona in another workspace only under a grant that names the pair.


**5. Keep blocked holds for the chat's life (V100-25).** The same chat asking across the same
pair again is refused at once with the person's no, and they are not asked twice. A new chat is
asked. It is the app's memory, gone with the chat, and it stands in for the question only: a
grant the person makes afterwards covers that chat like any other.


**6. Never for this pair (V100-25).** The Notice has a fifth answer. It is the person's, on
their machine, never committed, and it is the one deny among the records of dispatch.

- While it stands, no chat running as that persona is asked or allowed to dispatch to that
  target. No grant covers the pair: not one for a chat, not the person's, not the project's,
  not "any persona". Only a policy lock is said before it. Nothing in the project's file lifts
  it; the person does, in Settings.
- **It holds down a chain.** A chat with a chat of the refused persona anywhere above it in its
  lineage is refused that target too. This is a fail-closed reading the dispatcher chose in
  review, flagged for the operator: without it, work the refused persona asked for reaches the
  target through a third persona, and "any persona" on the persona in the middle makes that
  need no grant naming the target.
- **The person's own dispatch from a chat's tab is not held to it.** It is their rule for
  chats, as a grant is.
- **It is kept in a file of its own and fails closed.** A record that is there and does not
  read is never "no nevers": no grant covers any pair of two personas until it reads, the
  person is asked and told why, an unattended chat is refused, and a write refuses rather than
  replace what it could not read.


**7. The audit.** Beside a grant and its revoke the event log gains a never and its lifting
(#1503), **Not on my machine** and **Give back** (#1504), and an Allow that started one
dispatch and kept no grant (#1505). A grant for any persona is a grant whose target is `*`.
A grant purlis set aside is a revoke by the host, never by a person. Every one says the
workspace it holds in, and null for any (#1505); one made on a refusal kept while nobody was
there says where it came from (#1507).

**8. A persona may say what it wants, and that is never a grant (V100-20).** A persona's
definition may carry `wants`, the personas it usually works with.

- Nothing that decides a dispatch reads it: not the grant, the rule for an unattended chat,
  the limits or the loop rule. The list of who may dispatch to whom stays in the project's
  file and the machine's record.
- A chat can write persona definitions, so the line is an offer a chat may have written. Only
  a finished persona of the project is offered: never the persona itself, a name purlis keeps
  for itself or "any persona"; at most six; in alphabetical order.


**9. One answer may make several grants (V100-21).** Change 3 says the Notice "offers the
three levels" for the pair; an answer was one pair. It still offers the pair at the three
levels, and under the answers it now offers one box for each wanted persona that nothing
answers for yet.

- **A box is never ticked for the person.** An Allow keeps the asked pair and each ticked one
  at the level pressed. Each is a grant that names its pair, audited as its own, listed and
  revoked on its own.
- Keep blocked and Never for this pair are about the asked pair only.
- Not offered: a pair a grant covers, one the person said never to, one policy locks, one the
  person kept blocked in that chat (item 5: they are not asked twice, by a box either),
  one with a question of its own waiting (its brief is shown there), and any while the record
  of nevers does not read.
- **The answer is held to the question as it was shown.** The app builds the boxes and what
  each persona works with from the project's files, never from the request. The window sends
  back a digest of all of it. The files are read again at the answer: only a name still
  offered is granted, and where the question reads differently now nothing is granted or
  audited, the asked pair included, and the person is shown it again with no box ticked.
- No box makes "any persona" (item 4 holds).
- **Every ticked pair is kept with the answer's own workspace condition** (item 13): in the
  workspace the task works in for the narrower Allow, in any workspace for the wider one, and
  each audit says which. A box is offered where the pair needs a grant for a task in that
  workspace, so one granted only for another workspace is still a box. No box is offered
  where the task's workspace is not there yet: an Allow there keeps no grant.


**10. The question says what the target works with (V100-28).** For the persona asked about,
and for each box's persona, in purlis's words and from records no chat writes:

- the names of the vaults its chats are handed: the ones the vault registry tags for it and
  the ones the person let it use on this machine. Not the `vault:` line of its own file;
- the hosts the project's file declares for it;
- **the personas it may itself dispatch to**, from the grants in force for it: named ones, or
  "any persona", and nothing where there is none. The dispatcher added this in review of
  #1502, extending V100-28's sentence, and it is flagged for the operator: work handed to a
  persona can go on from it without a further question;
- a grant that holds in one workspace only is said with that workspace ("qa in runners",
  "any persona in runners"): it is reach all the same;
- each list is clipped to three with "and n more", and the digest covers the whole;
- **where the project's sandbox is off it says so of vaults and hosts alike**, since no list
  of either holds a chat there;
- never a secret's name or value. Allowing a dispatch does not allow the use of a secret:
  that consent is unchanged, and the question says so.

**The same words, from the one function, are said wherever a yes reaches a persona**: on the
Notice that says a teammate's grant arrived (item 14), where a digest of them is part of what
an answer is held to, and on a refusal kept while nobody was there (item 15).


**11. One table in Settings (V100-24).** Settings › Project › Dispatch lists each persona and
whom it may dispatch to, with where every grant comes from (this chat, me on this machine, the
project) and the action for that source, and it is the one place "any persona" is set and
cleared. Taking a grant back stops new dispatches only: a task already running is left as it
is. A project grant can be removed for everyone, which edits the committed file, or stopped on
one machine (**Not on my machine**), which does not. #1504 builds it. Under a
persona's name the table says what its definition wants (item 8), as a note that grants
nothing, and each grant's row says which workspace it holds in (item 13).


**12. A persona that goes away, and a persona of that name there again (V100-61).** A grant is
kept by name, so a name that changes hands must not carry what the person allowed the persona
that had it. Two rules, as #1504 builds them:

- **A grant is in force only while both personas exist.** It is checked where a dispatch is
  judged, and nothing is moved: a dispatch to a name that is no persona is refused, and so is
  one from a chat whose persona is no persona of the project now, whatever is granted its
  name. A named pair, "any persona" and the project's grants obey it alike. A branch without
  the persona therefore changes no record: the grants are not in force while it is away and
  are back when it is.
- **Away, then there again, asks once.** A name a grant holds that is seen to be no persona is
  marked, by a judged dispatch, by a read of Settings, or by purlis's own `persona remove`.
  When a marked name is a persona again with the very definition that was known of it, the
  mark is lifted and nobody is asked (a branch switched away and back). When it is a persona
  again with another definition, or purlis itself creates a persona under a name grants hold,
  the person's grants for the name are set aside, with what that machine had accepted of the
  project's, and are shown greyed with Remove. One **Give back** for the name returns them.
- **A never is not set aside.** It is the one deny, so it keeps holding for whichever persona
  has the name, until the person lifts it. Settings says where it was said of an earlier
  persona.
- **Reading Settings moves nothing.** The one thing a read writes is the mark.

**What this does not catch, stated so nobody takes it for more.** An absence nothing observed
still inherits: a persona removed and another made under its name, by hand or in one pull,
with no dispatch judged and no read of Settings in between, leaves no mark, and the new
persona has the old one's grants. Only an identity recorded in the persona's definition closes
that. purlis records none today; recording one is a change to a public format and is not
decided here. Nor is any of this a boundary against a chat: a chat that may write the
project's `personas/` can edit a persona in place. **purlis has no persona rename**: a persona
whose folder is moved is, to these rules, one that went away and another that appeared.


**13. A grant holds in one workspace, or in any (V100-27).** A grant carries one condition and
no other. #1505 builds it.

- **The workspace is the one the task works in**, never the one the asking chat is in: the
  workspace a dispatch names, the one whose repo its worktree is cut from, the one a handoff
  moves into, else the asking chat's own. Each is the app's own record, and the chat is then
  started in that place. A grant is the person's consent to what the other persona may be
  asked to do, and where that persona works is what bounds it; read the other way, a chat in
  one workspace could send the other persona to work in any. A task at the project's root
  works in no workspace, so no limited grant covers it. There is no narrower grant to offer
  there, so an Allow for the person or the project holds in any workspace, and the question
  and its two wider answers say so.
- **The question offers the narrower grant first.** Where the task works in a workspace, an
  Allow for the person or the project holds in that workspace only; *in any workspace* is the
  explicit other choice. A grant for one chat is for the task it was asked about: it covers
  that chat's dispatches in that workspace and nowhere else, and has no choice to make. A
  question asked again for another workspace says where the pair is already allowed. Keep
  blocked is not limited: it is per chat and persona, in every workspace.
- **"Any persona" may be limited the same way. A never is not**: it holds in every workspace.
  A refusal that stopped at a workspace's edge would be walked around by naming another place.
  The chain rule of item 6 and the loop rule are as they were.
- **A limited grant never reads as an unlimited one.** Each level spells it apart from the
  grants that hold everywhere: a table in the project's list (`{ to = "devops", in =
  "runners" }`), and keys of their own on the machine. A build that does not know the
  condition reads nothing there. An entry that does not read grants nothing. A grant written
  before the condition existed holds in any workspace, as it did. A teammate's limited grant
  waits for a yes of its own on each machine; a pair accepted for one workspace is not thereby
  accepted for all, nor the reverse, and a teammate widening a grant is told as a new grant.
- **A chat nobody is at crosses into another workspace** only under a grant that names the
  pair and covers the workspace it crosses into: one that holds in any workspace, or one
  limited to that workspace. "Any persona" still does not count there, **for a handoff that
  moves the work into another workspace as another persona as for a task** (the dispatcher's
  ruling in review of train 64, flagged for the operator). A crossing refused is kept for the
  person as item 15 keeps a refusal.
- **A workspace that goes away.** A limited grant counts only while a workspace of its name
  is there. A workspace made later under the name of one that was seen gone inherits no
  grant: the grant is shown as covering nothing, with Remove, and the person may count it
  again for the workspace that is there now. Nothing is moved by a read; the one thing a read
  writes is the count of times the name was seen gone. purlis's own workspace commands look
  too: `workspace remove` and `workspace rename` count the old name gone as they finish, and
  whatever makes a workspace folder looks before it makes one.
- **A grant does not follow a rename.** `purlis workspace rename` leaves the grants limited
  to the old name where they are: they cover nothing under the new name, and nothing in a
  workspace made as the old name later, until the person sets each one's workspace again.
  The project's file names the workspace for teammates too, so rewriting it is not that
  command's to do. The rename says how many it left behind.
- **No standing grant names a workspace that is not there.** An Allow on a question about a
  task whose workspace is not made yet (a handoff that makes it) starts that one dispatch,
  is recorded as that, and keeps nothing; the next dispatch asks. Accepting a project grant
  and setting a grant's workspace refuse such a name.
- **One start is for one dispatch.** Where an Allow starts a dispatch that no grant covers
  (the workspace is not there yet, or the list of nevers does not read), it is for the
  dispatch the person read, with that brief, and for nothing else. It ends when that
  dispatch returns, started or not, and goes when the pair's grant is revoked, said never
  to, or has its workspace changed.
- **What this does not catch:** a workspace's folder removed and made again by hand, or by a
  pull, with nothing looked at in between; a workspace has no identity beside its name.
- **Settings shows and changes it.** The table's workspace column says where each grant holds.
  Narrowing and widening a grant of one's own are each a confirmed, recorded write; a project
  grant's is changed for everyone, with the confirmation every write of the committed file
  has.
- **The audit says the workspace** of every grant, revoke and decline, and null for any.


**14. A teammate's grant is shown when it arrives (V100-62).** #1506 builds it.

- When the project's file comes to hold a grant the person has neither accepted nor declined,
  the window says so once, at its own level and not on a chat's tab: after a pull, a branch
  switched or a hand's edit, and when the project opens. Everything that arrived together is
  one Notice. Until it is answered nothing listed is in force for that person.
- **Named pairs** take **Accept** and **Not on my machine**, with a way to decide each in
  Settings.
- **"Any persona" is told, and never accepted, on a Notice.** Item 4 and V100-23 stand as
  written: it is made and accepted in Settings only. The Notice says it in its own sentence,
  with where it is accepted, and offers for it only Not on my machine and the way to
  Settings. The command behind the Notice refuses to accept one whatever it is sent.
- **Accepting is a machine's own act.** It records the acceptance and never goes through the
  writer of the committed file.
- **Anyone who can push can raise this Notice.** So a re-ask never carries a new grant (each
  has its own Accept), Accept leads only where the line names every pair it accepts, and
  putting the Notice away answers nothing. The question on the tab of a chat that needs the
  pair stays as the fallback, says the same thing, and takes the same two answers.
- **An answer is for what was shown.** Only a grant still exactly as it was listed is
  answered; where the list moved, the person is told and shown it as it is. What the target
  works with is said beside each pair (item 10) and is part of how a grant is listed, so a
  yes that comes after the target gained a vault, a host or an onward grant answers nothing.
- **A grant limited to one workspace is not told here yet.** It waits in Settings with its
  own Accept (item 13).
- A grant naming a persona the checkout does not define is said to cover nothing and is not
  accepted. An accepted grant counts only while both its personas exist (item 12), and one
  naming a name that changed hands waits again on this Notice, saying so.
- **A grant limited to one workspace is bound the same way** (item 13): accepted on this
  machine, it is dropped where a commit took it out and asks for a new yes, it is in force for
  nobody while the history cannot be asked, and a branch without it drops nothing. What was
  accepted for a name whose grants are set aside (item 12) is bound too, so a Give back never
  restores a grant a commit took out meanwhile.
- **A yes is bound to what it accepted, by the project's history.** A file that was changed
  and changed back reads the same, so the acceptances are kept with the commit they were last
  checked through. An acceptance is dropped where a commit since then **took the grant out**:
  it lacks the grant and a parent holds it, read on every merged line and under every name
  the project's file may have. The person is told once that it was taken away, and a grant
  put back waits for a new yes and says why. A decline goes the same way.
- **Absent from the working file is not a removal.** Such a grant is not in force, and that
  is worked out at each read; nothing is dropped or told, so a branch switched away and back
  moves nothing, and no read of Settings writes a drop.
- **Fail closed.** Where the history cannot be read (the kept commit gone or unrelated, too
  many commits, an object missing or too large, grafts, a shallow boundary inside the range,
  git out of time) every acceptance is dropped and asked again, saying that is why. Where
  nothing answers at all (git cannot be run, the commit checked out cannot be read, the
  record cannot be written) nothing stored changes and no accepted project grant is in force
  until something does.
- **The limits.** A history rewritten so that no commit this machine has took the grant out
  (a force-push to a line that never lost it) keeps the acceptance: the check is over the
  history as pulled. A hand's uncommitted remove-and-restore is not seen. A project in no
  repository has no history to read. An acceptance kept by a build from before this is bound
  at its first check and not asked again; an older build that rewrites the record strips the
  binding, and the next check binds afresh.


**15. A refusal with nobody there reaches the person afterwards (V100-29).** #1507 builds it.

A chat nobody is at still dispatches under standing grants only, and is still refused at once
where none covers the pair: no question is put to it, nothing is held, and no Notice is raised.
What is new is after the fact. A refusal that was only for lack of a grant between two personas
is kept (one entry a pair and workspace, with a count; capped; 30 days), and the title bar's
needs-you list says "`<persona>` wanted `<persona>` while you were away", with **Dismiss**,
**Never for this pair** and **Allow from now on**.

- **Allow from now on is the person's own standing grant for that one pair**, for them on this
  machine, audited as theirs with where it came from. It is the only level offered there:
  never the project's, never "any persona". It starts nothing; the dispatch that was refused
  is gone.
- **It is limited to the workspace the refused task would have worked in** (item 13), which
  the entry keeps from the app's own record of the dispatch: never the asking chat's, where
  the two differ. A refusal at the project's root has no workspace to limit a grant to, so
  there the grant holds in any workspace, and the item says so. For a name that is no
  workspace of the project by the press, nothing is recorded and nothing is kept. An entry
  is answered, and settled by a grant made elsewhere, for its own workspace alone.
- Its reach is that grant's, and the item says it before the press: where it holds, that a
  chat nobody is at may use it too, and what the target works with, in the question's own
  words (item 10).
- **Dismiss holds.** The entry is kept and marked; a chat asking on is counted and not
  listed, and is not told whether its refusal was listed. It is listed again only by a refusal
  seven days or more later. **Never for this pair** is the store's never.
- The item is attached to no chat. The asking chat does not count as needing the person.
- A never, a policy lock, a limit, the loop rule and every other refusal no grant mends are
  not kept. A pair the person said never to by the time they look is dropped without a word.
- **Nothing a chat wrote is kept or drawn**: not the brief, and not the task's name. The item
  is two personas, a workspace, a count and a time, all the app's own.
- **A refused chat cannot move a row under the pointer.** The list is in the order pairs were
  first refused, the window draws the list it was opened on until it closes, and the keyboard
  arrives on Dismiss.
- The list and its answers are the window's alone: served on no link, named by no hook line.

**16. A chat names only a profile the project lists for the target persona (V100-60).**
#1509 builds it; the project's format (`[dispatch.profiles]`) and the refusals are written in
`docs/plane-format.md` and `crates/purlis-core/docs/handoff.md`. A chat a dispatch started is
held to it, and to the rule that a dispatched chat never runs with its harness's prompts off,
each time it is started again: at a relaunch, a restart, and a Reopen of its finished task.
The loop rule holds under every grant, "any persona" included.

**17. The commands of who may dispatch to whom are the window's alone** (the dispatcher's
ruling as train 64 was joined, for the operator to confirm). Allow, Keep blocked and Never on
a dispatch's Notice (every answer of the person's, the one that keeps nothing included), the
commands of Settings' table, the answers to a teammate's grant arriving and to a refusal kept
while nobody was there, and the reads of what stands, what waits, and those two lists, are
served to the person's own window over its own channel and on no link, whatever scope a link
holds. The link's client is generated without them.

**18. The chain is read from the record, not from the chats still open (V100-58).** #1521
builds it. A task's record of open chats keeps the personas above it (`chats[].from.above`,
`docs/plane-format.md`), written by the app when it starts the task, from its own record of
the asking chat: the asking chat's persona, then the chain that chat's record keeps. The loop
rule and the person's never for a chat above read it, so a chain holds where a chat in the
middle has finished, closed or been cleared. Nothing a chat sends goes into it. A finished
task reopened as an ordinary chat names no asking chat and starts a new chain.

- **A record written before the key fails closed.** Its chain is read from the chats still
  open, as before. Where that walk meets a chat that has closed, purlis cannot say who was
  above, and refuses as if any persona were: every dispatch to another persona, and one to
  its own where the person said never to any persona dispatching there. The refusal says that
  the chat's chain began under an older version, which kept no record of it. Such a chat's
  tasks keep no chain either, and are held the same way.
- **Before refusing, the chain is read from the dispatch records (#1548).** Who dispatched
  whom, by each chat's id and never its number, and the persona each ran as: the app's own
  records, held to what it writes. It is taken only where it is whole: it reaches a chat
  whose record keeps its chain, or the chat the person started, which, where the asking chat's
  record names the lineage's first chat (`root`), must be that one by its id. Where the records
  stop at a chat that has closed, the chain is whole only if that chat is the lineage's first
  by the asking chat's `root`, and as deep as the asking chat's record says. **A depth alone is
  never enough**: a chat dispatched below one from before the depth key was given a depth that
  is short, and names no root. Records that disagree, one the app would not have written, a
  loop, records that stop anywhere else, and an asking chat that names no root with records
  that stop each leave the chain unread, refused as above.
- **An older ancestry from before the depth key keeps its whole chain (#1548).** Where the
  chats still open show a chain longer than the asking chat's recorded depth, the depth is
  raised to it (deeper is the safe side), and a chat it starts keeps the whole chain.
- **The dispatch question offers no box the decision would refuse (#1548).** A wanted persona
  the person said never to for a chat above the asking one is not offered; for a chat whose
  own record keeps no chain, a persona any never names is not offered.
- **The asking chat is read under the lock the decision is made under.** One that has closed
  or ended by then is refused, so a chain read short is never written into a new chat.
- A kept chain whose length is not the record's depth, or that names what cannot be a
  persona's name, reads as no chain kept: never as a shorter one.

Still open, and not decided here:

- telling a teammate's grant limited to one workspace, and a narrowing of one, on the Notice
  of item 14, with a recorded decline;
- an identity for a persona and for a workspace beside its name, which is what closes the
  limits items 12 and 13 state;
- a history rewritten so that no commit this machine has took a grant out (item 14);
- a task's request named with its whole path, and one question for several tasks at the same
  block (V100-56, V100-57; #1508): not in train 64.

### Rulings added in review of #1512 (2026-10-09, delegated to the dispatcher; the operator is told)

- **A task's time limit counts its working time only.** `minutes-per-task` (§4's new limit,
  V100-59) counts a task's time while its turn runs and it shows no prompt: never while it waits
  on the person, never while it waits on its own tasks, and never while purlis is not running.
  The app keeps the total on the task's dispatch record, so **a task put back after a restart
  keeps the working minutes it had**, and the night the app was closed adds nothing. Past the
  limit, the task and everything below it are asked for their report, the way Stop and get its
  report asks, in purlis's words and never as the person's stop.
- **The clock never stops a task the person is in the middle of.** A task showing a permission
  prompt or a question, or one the person has typed into since its harness last spoke, or one
  with such a task below it, is left for the next look.
- **A session's token limit is read and shown, and not enforced yet.** The figure is relayed
  through a file a chat can write, for its own conversation or another's, so a limit that
  refused or stopped by it would let one chat stop another's work. `tokens-per-session` is
  shown on the session's row against its figure with "not enforced yet", and refuses and stops
  nothing until the figure is out of a chat's reach (issue 1457). This replaces §7's "Cost is
  shown, not enforced" for tokens only in that the limit can now be set.

*Amended 2026-10-12 (#1512, #1457; decided under delegation in the dispatch of #1512, the
operator's confirmation pending):* **the session's token limit is enforced.** The figure moved
out of a sandboxed chat's reach: each chat's is kept by the chat's own id in the app's folder,
which no sandboxed chat can write, written only by the harness's status line outside the
sandbox. A chat with no sandbox runs as the person and can still write it, as it can every file
of the app's folder, so the limit is a brake and not a boundary. At
`tokens-per-session` a new task of the session is refused with the figure and where the limit
is changed (a handoff begins a session of its own), and the session's tasks at work are asked
for their report by the time limit's stop, each told its session's figure; the clock still
never stops a task the person is in the middle of. A harness that reports no tokens is still
not counted. Cost in money stays shown, not enforced (§7).
