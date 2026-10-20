# Amended 2026-10-20: a runner chat's withdrawn ports are given back after a re-link

**Built by #2625** on item 4 of the amendment of 2026-10-18 for #2511 (D-2511-4), under which a
port of the runner's machine withdrawn when the last link a chat's confinement was named to
ended stayed withdrawn after the window took the chat back. What follows was decided in #2625
and is marked *decided in implementation, to be confirmed*.

1. **A port the runner withdrew because no window was left is given back as the window takes
   the chat back.** The runner keeps which of a confinement's given ports it withdrew as its last
   link ended, and once a link of the chat's window names it again (`take_back`) it carries each
   again as the plan gave it: the person's answer that gave it stands. Another window's link
   gives nothing back. A port the proxy cannot carry again stays withdrawn, and the runner says
   so in its log. *Decided in implementation, to be confirmed* (D-2625-1).
2. **A port the person withdrew stays withdrawn.** A port the window withdrew from the chat is
   recorded as withdrawn in the confinement's plan and is never one the runner gives back,
   whenever the window's withdrawal lands. *Decided in implementation, to be confirmed*
   (D-2625-2).
3. **A withdrawal and a give-back never cross.** The runner's own withdrawals as a last link
   ends and its give-backs run on one lane, and each acts only on the ports still withdrawn for
   want of a window, so a re-link that comes back before the withdrawal ran leaves the port
   carried. A window's withdrawal marks its port withdrawn, and no longer one to give back,
   before the proxy hears it; a give-back checks, gives back and marks each port under the same
   lock. Whichever lands first, a port the person withdrew ends withdrawn. *Decided in
   implementation, to be confirmed* (D-2625-3).
