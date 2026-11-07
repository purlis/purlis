# Amended 2026-11-06: an ask ahead that names a reviewed program

The person's ruling, 2026-11-06, on #5244. A `secret_ask` (or `purlis secret ask --program`) that names the
program the chat will run has no command yet, so there is nothing exact for Allow once to bind, and
the variables the key will be handed by aren't known.

- **A reviewed program (curl alone, for now)** gets one notice: the key's value field, and **Allow
  for this chat**, bound to the chat, the program as the app resolves it, the vault and exactly
  those keys. The first run it covers fixes the variables they are handed by, and from then on the
  grant covers only those. No Allow once, and no Always allow, ahead of a run.
- **Any other program** gets Fill only, and its run asks for the Allow on the exact command, as a
  run does. **A program that runs what it is told** gets Fill only, saying why there is no Allow.
- **A program the person did not allow with that vault in this chat** is refused at once, with
  nothing asked.
- Every check a run makes holds for these runs too: the program is resolved where no chat writes;
  no key a browser login, a fill line or a `mint` line holds is handed to it; a granted run starts
  without the chat's hosts, ports and certificate authorities; policy and the scopes it leaves come
  first; and the first run's variables pass the same checks as any run's.

A chat is told to prefer simply running the command (the session-start line, the `secret_ask`
tool and the purlis-secrets skill, in the same words), since a run gathers every gate into one
notice (#5242). What a filled answer says promises only what will work: filled and allowed, filled
but not allowed, or filled with the run asking for the Allow.
