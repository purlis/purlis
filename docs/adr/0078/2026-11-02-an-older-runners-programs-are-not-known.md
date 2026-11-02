# Amended 2026-11-02: an older runner's programs are not known

**Built by #4684**, on the amendment of the same day, *a chat is found on the pool it ran on*
(#4590), whose point 2 said a runner before session protocol 1.67 is answered for by the relay with
no chats, so its records went at the first put-back though it was never asked. And on the
amendment of 2026-11-01, *a restart takes back a draining runner's chats*, whose point 8 has the
relay not ask such a runner.

*Decided in implementation, to be confirmed* (D-4684-1 to D-4684-3). The person's ruling of
2026-11-02 holds as it stands, and this is its third part: a record is kept while the answer is
unknown.

1. **Session protocol 1.68 says it.** A relay still does not ask a runner before 1.67. To a window
   of 1.68 or later it refuses `programs_of` with `programs_not_known`, naming the runner's
   version; to an older window it answers no programs, as before. *(D-4684-1)*
2. **The window keeps what it knows.** That refusal leaves what the runner runs not known: a put
   back settles no record of a chat on it, and the next one, on a runner since updated, does. Such
   a runner takes back no chat in any case. *(D-4684-2)*
3. **Its pool is marked no more than before.** Any other refusal of `programs_of` from a pool's
   runner counts as one that may have run a chat outside the sandbox (D-2050-27). This one is the
   relay's own word that it did not ask, of a runner that was answered for with none until now and
   so never marked: marking it at each restart is left to a decision of its own, not made here.
   *(D-4684-3)*
