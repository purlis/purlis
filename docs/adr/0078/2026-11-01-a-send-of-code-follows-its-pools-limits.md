# Amended 2026-11-01: a send of code follows its pool's limits

Amends §7's fixed bound on a transfer of code. The person's rulings on a pool's limits (spec #4275,
Q7, Q9, Q13) make the largest code sent and the longest code send a pool's limits.

1. **Only a send moves.** `codeSize` and `codeSendTime` hold a send to a runner, on both ends: the
   desktop tells the host what it gives this send (`held`, session protocol 1.65), up to 64 GiB and
   half a day whatever it asks. A value comes only from this machine's own `runners.json`, set by the
   person.
2. **What a runner writes back never moves.** A bundle or a conversation from a runner stays held to
   1 GiB and 10 minutes on the desktop, so raising a limit widens nothing a runner can make this
   machine take.
3. **What is offered is this machine's measure.** The size a suggestion comes from is the bundle as
   this machine measured it before anything went, and a send out of time is judged by this machine's
   clock, never by a runner's words or numbers.
4. **Only a wait changes.** Every check on what a bundle may carry stands, whatever its limits are.
