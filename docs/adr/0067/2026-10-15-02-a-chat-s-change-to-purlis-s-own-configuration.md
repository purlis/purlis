# Amended (2026-10-15, #1975; the operator's rulings PC-1..PC-14 of 2026-10-14): a chat's change to purlis's own configuration is a proposed change the person applies

Ruled by the operator. A chat may not write the files a later program loads settings from
(section 5); for purlis's own configuration its edit becomes **a proposed change**, which only
the person applies.

- **Which files** (PC-1, PC-11): the project's manifest at its root and in each workspace, a
  persona's `mcp.json` and `persona.md`, and a `.claude/skills` folder's files at the root, in a
  workspace or in a persona. Plane only. Below `.claude/skills`, skill files alone. Files that
  run as code the moment they are there stay the person's to change, and a repo clone's files go
  through its own review.
- **How it is proposed** (PC-2): a sandboxed Claude Code chat's own `Write`, `Edit` or
  `MultiEdit` of such a file is refused by its edit hook and handed to the app as a proposal;
  another harness runs `purlis propose <file> --from <draft>`. **What travels is the change**:
  an edit's own replacements, or a whole new content, with the hash of the file as it was read.
- **Made and written by the app** (PC-3): the app reads the file itself, only as the one
  regular file its name says, makes the change to it, shows the file before and after, and on
  **Apply** writes exactly that, as the person, only while the file is still what the proposal
  was worked out against. Nothing the chat sent is taken for what the file holds.
- **Audited** (PC-4): `trust.proposed.apply`, once before the file is written (`intent`) and once
  after (`applied`, `stale`, or `failed` with why). The file then rides the person's next save
  (PC-12: git history and Changes' discard undo it).
- **Apply judges again**: the change is made to the file as the app reads it, shown whole by the
  row's own rules, and written under one lock per project held from the read to the rename, so
  of two Applies against the file as it was, the second is stale.
- **No new persona or workspace** (the dispatcher's decision, ruled by the operator, 2026-10-15;
  new skill folders only): a proposal makes no folder but a skill's (`.claude/skills` and
  below). A file in a persona or a workspace that is not there is refused; a new persona or
  workspace is made with purlis's own ways first.
- **Allows nothing** (PC-5): a tool or host the change declares still asks its own Allow.
- **Stale** (PC-8): a file changed since its proposal is not written; the proposal offers
  Dismiss alone and the chat is told to read it again and propose again. Never merged.
- **One a file a chat** (PC-9): a newer proposal replaces the chat's older one for that file;
  applying one makes the others to that file stale.
- **The chat hears fixed words** (PC-10): applied, dismissed or stale, on its next turn. Its
  proposals end with it.
- **Shown whole** (PC-7): Apply is offered only beside a change the window shows whole. A change
  too long for a row offers **Open**, which opens its own tab: the change whole in the comparison
  view, with Apply there. Apply is judged again by the rules of where it was pressed: from a row,
  only a change a row shows whole; from the tab, also one too long for a row. A change not shown
  for anything but its length (a credential's shape, bytes) is applied from nowhere.
- **Never "always apply"** (PC-13).
- **Policy** (PC-14): `sandbox.proposed-changes = "off"` holds none, and a chat's edit is refused
  as before.
- **Live check, for the operator**: a hook runs before Claude Code's own deny rules, so the edit
  hook sees an edit those rules deny. To be confirmed on a live Claude Code chat.
