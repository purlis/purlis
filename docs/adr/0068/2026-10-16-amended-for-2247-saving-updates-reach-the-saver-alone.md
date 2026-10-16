# Amended for #2247, how saving goes reaches only the link that may save, 2026-10-16

The amendment for #2002 makes saving a held project the window's alone (its item 2, `Power::Save`)
and pushes the host's question of who is mid-turn to the window's links alone. #2247 gives what
each look did the same audience. This note adds to that amendment's items 2 and 5; nothing else
changes, and the session protocol does not (no key, command or version).

1. **Only the link that may save a project hears how saving it goes.** The pushed `saving` and
   `who_is_mid_turn` reach a subscribed link whose scope may save (`local-ui`) and that holds the
   project. Every other scope hears neither, `remote-link` included, whether or not it holds the
   project: a relayed hold does not carry saving, so it does not carry how saving goes either.
   *D-2247-1, to be confirmed.*
