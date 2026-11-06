# Amended (2026-11-06, #5492): bring back, move, export and Delete now are proposed too, and an export goes where the person picks

The verbs [the amendment of 2026-11-06 for several skills](2026-11-06-a-chat-proposes-a-change-to-several-skills.md)
left for later, under its rule: a change is made directly only at the person's interactive
terminal, with no flag to skip its question, and every other caller only proposes it as
`Ask::ProposeSkillSet`. Who is the person is decided the same way, by the same checks.

- **The verbs.** `purlis skill restore` and `mv` take several names, and `mv` and `export` take
  `--source <url>` too. `purlis skill delete-now <names…>` is new: it takes skills off the list
  `restore` prints before their time is up. The ask names them as `SetChange::Restore`,
  `SetChange::Move`, `SetChange::Export` and `SetChange::DeleteNow`. A change of one skill by
  `restore` or `mv` from a chat is proposed too, as `rm` of one is. `mv --name` and
  `restore --as` stay of one skill, and are refused with several.
- **Delete now asks first, even at the terminal, even of one skill.** It is the one change to
  skills with no undo. With no terminal to ask at, it is proposed, whatever the caller.
- **A skill in Deleted is named by its level and name.** The app judges it against what the plane
  lists in Deleted when it is shown and again at Apply. It leaves out one that is not listed, or
  one listed more than once at that level, which only the window's Deleted list tells apart. Its
  entry there is part of the state a proposal is held at, so one brought back or taken off since
  makes it stale.
- **A move carries its grant in words**, read again when it is made. A skill that would go into
  the project's skills with no grant is kept out, and the rest move.
- **An export carries no place** (the person's choice of 2026-11-06). The app writes it outside
  the chat's sandbox, so a place the chat named would be one a chat picks for the app to write.
  The ask names the skills and whether it is a folder or a zip, and nothing else. At Apply the
  window asks the person where, in the save or folder dialog Select's Export uses. A place not
  picked writes nothing, and the proposal stays held. A stale one is refused before the dialog
  opens. An export of several skills with `--to` is refused anywhere but the person's interactive
  terminal, a chat's or a script's alike, and says why. At the person's terminal, `--to` is where
  it goes, after the question. An export of one skill is as it was: it writes where its caller
  can, and is no change to skills.
- **Restore and Delete now take no `--source`.** A deleted skill keeps no record of the source it
  was installed from that purlis reads, so the names are the only way to name one.
- **The boundary is unchanged.** The sandbox is the boundary for a sandboxed chat, and the check
  stops `purlis skill` from making the change, and nothing more, as the earlier amendment says.
