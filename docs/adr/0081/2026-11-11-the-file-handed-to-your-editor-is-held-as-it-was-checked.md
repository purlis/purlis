# Amended (2026-11-11): the file handed to your editor is held as it was checked

From #3185 and #1044's first line. It adds to **Open in your editor at file:line** (RC-20): what
purlis launches is still the file it checked.

- **The check and the launch are one held file, not two reads of a path.** Every route that hands
  a file to your editor (a branch's file, a workspace file, the operator's withheld `AGENTS.md`,
  a blocked save's file, a harness declaration and the extension record) opens the file when it
  checks it, through no link from the folder it trusts, and holds it
  (`purlis_core::youreditor::Checked`). The app gets what to launch only from `Checked::launch`,
  right before it opens the URL or starts the program. That opens the path again the same way,
  and it must be the same file on disk (device and inode). A link put on the way, or another
  folder or file put in the checked one's place, launches nothing, and the window says the file
  changed after purlis checked it.
- **The editor's own open is narrowed, not closed.** An editor reached through its URL scheme
  opens the file itself, later, by its path, and so does `$VISUAL`/`$EDITOR` once started,
  detached or in a shell tab. purlis cannot hold the file for it. Handing a program the held
  descriptor (`/dev/fd/N`) would close the gap for that route alone. It also breaks what an
  editor does with a path: its title, its backup and swap files, and a save that writes a new
  file and renames it over the old. So a swap between the second check and the editor's open still
  reaches the editor. What crosses it reaches only the person's editor, never the window or a
  model.
- **Off Unix only the walk is asked again.** The standard library has no stable file identity
  there, so a link on the way is still refused, and a file put in another's place with no link is
  not caught.
