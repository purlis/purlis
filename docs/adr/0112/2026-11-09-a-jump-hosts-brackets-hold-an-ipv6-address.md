# Amended 2026-11-09: a jump host's brackets hold an IPv6 address

Decided under the operator's standing order to take the recommended path (D-5892-1, #5892, from
the ninth adversarial review of #5876), recorded here as a delegated decision. It follows *A Run
grant refuses a flag that names a command* (2026-11-09).

ssh pastes a jump list into the command its shell runs without quoting it, and the hops before
the last keep their brackets there, where `[...]` is a glob bracket expression. A host in brackets
in a value ssh pastes (`-J`, `-o ProxyJump`, `Hostname` and `User`, and a jump's destinations) is
let through only when what is inside is spelled as an IPv6 address is: hex digits, `:` and `.`.
`-J a,[.-_]` and `-J [g::1]` get no Allow, and neither does a user holding a bracket (`-J [x]@host`), which ssh pastes as `-l [x]` (D-5892-2, from #5929's second review). Such a glob could only have matched a one-letter file
name and is never read again as shell syntax, so this is tightening, not a fix of a way in.
