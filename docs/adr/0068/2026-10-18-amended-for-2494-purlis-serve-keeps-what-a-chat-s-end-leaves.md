# Amended for #2494, `purlis serve` keeps what a chat's end leaves, 2026-10-18

Spec #2041 (ruled 2026-10-14) asks that a runner chat behave exactly like a local one, and a
runner is `purlis serve`. This note adds to the amendments for #2003 and #2447; everything else
stands as accepted. Each item is **decided in implementation, to be confirmed** (D-2494-1 and
D-2494-2).

1. **Both binaries' hosts keep a program's end with one piece of code.** The host's `Record`
   (a write the window decided, what a chat's program's end leaves, and what a chat the last
   host left running leaves) is the core's event log (`purlis_core::hookchannel::EventLog`),
   the same value the project's hook channels write to. The window's host and `purlis serve`
   each record with it, so in either one a chat's end drains its spool, ends its sub-agents'
   runs in the state and for the cause its end gives (ADR 0076 §2, §6), and, where no window
   held its project as it ended by itself, ends the dispatch it worked as one that went without
   its report (the amendment for #2003, item 6). How an exit ends a run is the core's too, and
   the window's own path reads it from there. *Rejected:* a second `Record` in the `purlis`
   binary, which would be two copies of one rule. *D-2494-1.*
2. **`purlis serve` writes every entry the window's host writes.** A start it composes by its
   names writes the run it begins and its trust event (a person's opt-out among them) in its
   own log, as the window's host does (the amendment for #2171), and `record` from a window on
   its own machine is answered there as it is in the window's host. `record` stays `local-ui`
   only (the amendment for #2003, item 3), so a relayed link cannot write in a runner's log.
   *D-2494-2.*
