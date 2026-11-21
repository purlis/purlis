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
