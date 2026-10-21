# Amended for #2838, a runner's journal never names a vault, 2026-10-21

**Built by #2838** on the amendment for #2753, which has `purlis serve` keep the core's `info`
lines on its standard error: a runner's journal or its container's log. What follows was decided
in #2838 and is marked *decided in implementation, to be confirmed*.

1. **A host that logs as a service names the program of a brokered `secret exec`, never its
   vault.** Under `purlis serve`, the line of a run says the chat, its persona and the program,
   and a refusal says the chat and the program, without the refusal's words, which name vaults.
   The window's own log, which only the person reads, still names the vault. *Decided in
   implementation, to be confirmed* (D-2838-1).
