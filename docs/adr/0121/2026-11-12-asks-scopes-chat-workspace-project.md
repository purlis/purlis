# Amended 2026-11-12: Asks is scoped to Chat, Workspace and Project, and the ✋ counts the project in front

Agreed with the person on 2026-11-12, after a grilling and a prototype of the look (draft #6330,
`app/src/prototype/AsksScopes.prototype.tsx`, not for merge). It reverses decision 2's ✋ count
as decision 4 records it, narrows decisions 6 and 9 to the scope in view, replaces the scope
switch of [the amendment on other projects' asks](2026-11-10-another-projects-asks-are-answered-in-place.md),
and replaces decision 24's first pulse with the reminder bounce below. It answers #1921's first
box; its second box is moot.

## Why

- **"We are under a project, why show other projects?"** The person works in the project in
  front. A ✋ count and an Asks list that mix in every open project's asks answer a question the
  person was not asking, and make the ✋'s number change with work they cannot see. Each project
  tab already stands on the rail; it carries its own count, so nothing elsewhere goes unseen.
- **Alarm fatigue and habituation.** A cue that fires for everything, or repeats the same way at
  the same pace, stops being noticed: people learn to ignore it, and then miss the one that
  matters. So the count the person sees is the one that concerns them, the one movement that
  calls for attention is rare and bounded, and its repeats slow down. Urgency comes from the ask
  itself (an Expiring ask's sound, decision 12), never from a timer.

## The decisions

1. **The ✋ counts the project in front** (reverses decision 4's "every open project's", and the
   "+N elsewhere"). Each project tab on the rail shows its own project's count. **The Dock badge
   stays the total** across every open project of every window (#5999), since the Dock is not
   under any one project. Notifications, Quiet and the Expiring sound are unchanged and still
   span every project; a notification's click brings its project forward with the ask open, as
   before.

2. **Asks has three scope chips: Chat, Workspace, Project**, narrowest first, in a segmented
   control at the top of Asks, beside Quiet now.
   - **Chat** is the focused chat and its whole dispatch subtree (the chats it dispatched, and
     theirs). Its tooltip names the chat and how many tasks it holds ("steward 8 and its 2
     tasks"). With no chat focused, Chat is greyed and its tooltip says why; Asks then lists the
     next scope offered.
   - **Workspace** is the workspace in front on the strip. It is not drawn where the project has
     one workspace, since it would list what Project lists.
   - **Project** is every workspace of the project in front.

3. **All projects is dropped from Asks.** Another project's asks are answered in its own Asks,
   reached from its tab or its notification. The other places that share the scope switch (the
   Browsers grid, Skills, Views) rename their scopes to Workspace and Project and keep their own
   All, since a list of browsers or skills across projects answers a question the person asks.

4. **Asks always opens on Chat.** A pick lasts until Asks closes and is not kept per project
   (the kept `inbox.scope` is no longer read or written for Asks). An empty scope never switches
   by itself: it says "N waiting in this workspace ›" (or this project), one press to widen.

5. **Each chip shows its count** in the ✋'s colour, only above 0. The counts nest:
   Chat ≤ Workspace ≤ Project, and Project equals the ✋.

6. **Small round counts.** Every count (the ✋, the project tabs, the chips) is a small 1:1
   circle holding one character: 1 to 9, then 9+, so the circle never changes shape. On the ✋
   its centre sits on the icon's top right corner, and on the ✋ and the tabs it has a 2px ring in
   the ground beneath it. What a screen reader is told is always the real number.

7. **A changed count slides once** (about 200 ms, a motion token) on the chips, the ✋ and the
   project tabs. Under reduced motion the number just changes. A count drawn the same (9+ to 9+)
   does not slide.

8. **Answer once for all and keyboard triage act within the scope in view** (narrows decisions 6
   and 9). Identical requests group only within the listed scope, and Allow for all answers only
   the asks listed. ⌘⇧I opens Asks on Chat and walks the listed asks; past the last it goes back
   to the first, and it never moves into a scope the person did not pick.

9. **Notices and updates under the chips follow them.** A chat's Notice or update is listed in
   every scope that holds its chat. One that belongs to no chat (a persona's tools, the doctor, a
   stale clone) is listed only under Project.

## The ✋'s reminder bounce: the one movement allowed

The ✋ is the only thing in the window that moves to call the person. Nothing else bounces,
pulses or shakes; the chips and the tabs only slide their numbers.

- **On a new ask**: when the ✋'s count goes up, it bounces 3 times.
- **As a reminder**: while a Blocking ask waits in the project in front, it bounces 3 times again
  on a backoff of **5, 10, 20, 40 minutes, then every 60 minutes**.
- **Resets**: the backoff starts again at 5 minutes on a new ask, when the person answers
  anything, or when they open Asks.
- **Stops** when nothing waits in the project in front.
- **Holds**: no bounce during Quiet (its schedule or Quiet now), while the person types in a chat,
  under reduced motion (the number just changes), or while the purlis window is not in front. A
  held reminder is not made up for later; the schedule simply goes on.
- **Expiring asks keep their own sound** (decision 12) and do not change the schedule.
- **Remind me**: a setting in Settings › You › Attention, **Off / Gently / Often**. Gently is the
  backoff and the default; Often is every 5 minutes, fixed; Off is no reminder (the bounce on a new
  ask stays). It is kept on this machine, in the layout file, beside the Expiring sound.

This replaces decision 24's one pulse the first time a Blocking ask arrives.

## Consequences

- The amendment on other projects' asks keeps its plumbing (each window tells the others its
  projects' asks, ask by ask): it now feeds the project tabs' counts and the Dock badge. Its
  rows of another project's asks in this Asks, its cross-project groups and its All projects
  default are gone.
- `inboxScope.ts`'s `"all"` scope and its default go from Asks; `scopeDrawn` and `widerWaiting`
  read Chat, Workspace and Project.
- The Remind me value is one more key of `layout.json`'s `notifications` (Machine, syncable);
  `docs/plane-format.md`'s row names it before it ships (ADR 0069). Nothing else new is stored.
- Not chosen: keeping All projects as a fourth chip (it answers a question asked under no
  project, and the tabs and Dock already answer it); a reminder at a fixed pace by default (it
  habituates); a reminder that moves anything but the ✋, or sounds (decision 12 keeps sound for
  an Expiring ask); remembering the last pick per project (the person asked to start from the
  chat they are in).
