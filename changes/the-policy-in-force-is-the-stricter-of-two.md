### Changed

- **A tightening of the project's policy applies at once, and a loosening waits for your press.**
  Every decision is now the stricter of what the project committed and what you accepted (the
  shipped default until you first accept), and what purlis sees tighten is kept on this machine,
  so moving the project's last commit back, committing a broken file or deleting something a rule
  names no longer drops a forbid without your press. A tightening in your own local file applies
  at once too, and taking it out changes nothing until you press (M75-14, #7023). Starting a chat
  is held to the same accepted copy, and renaming or removing a persona or workspace folder
  without committing it drops no forbid.
- **`asks` is held on its own.** Where you accepted `asks` on and a commit turns it off, an ask on
  a read, write, connect or dial is still asked, even where the action's own family is not
  enforced (#7222).
- **A start is refused where its policy cannot be judged, and held to what was accepted even
  after a commit past the floor's cap.** In that state, a start is still judged by the accepted
  copy and every kept version, so a forbid that only they hold is not dropped; an entry that does
  not read, or a start with known gaps where nothing could be verified, is refused rather than
  let through by the shipped default. A last commit that git could not read steadily is refused
  too, instead of being read again without the plane's workspaces. Kept versions sealed by an
  earlier build do not read until you press again. Where there is no keyring there is no floor at
  all: the policy in force is the project's last commit and the shipped default. The project
  template `strict` is not stricter than the forbids it drops, which an accepted copy still holds.
  Where nothing could be verified at all (a `.git` swapped, say), a start is also refused with the
  sandbox off, and a known gap counts whatever mode its family is set to. The limit that remains:
  there, a forbid you accepted on a start for a specific profile, persona or workspace is not
  held, since no accepted copy can be read, and the chat that does start runs under the strict
  deny-state sandbox. That refusal is for a cause a chat can reach (the project's `.git`, the
  store's files, the registry); where only your keyring is at fault (it did not start, it stays
  locked, older nevers are not imported yet) or the project is not under git at all, a start is
  judged as it was before.
