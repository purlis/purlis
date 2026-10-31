### Fixed

- **A sign-in on this machine keeps its Ask while the harness cannot tell.** Its Ask could leave
  the Inbox while you were still signing in. It now stays until the harness says the profile is
  signed in, or you leave that sign-in, as a sign-in on a pool does (#4223).
- **Leaving a sign-in is seen when the shell's program ends.** Running `exit` in the shell now ends
  the watch and puts the Ask away, as closing its tab does (#4202).
- **An Ask raised just after you left a sign-in stays.** A chat refused in that moment keeps its
  Ask rather than losing it to the sign-in you had left (#4203).
- **Every project's window hears of a sign-in on a pool.** Another open project no longer keeps
  drawing an Ask that has already gone (#4205).
