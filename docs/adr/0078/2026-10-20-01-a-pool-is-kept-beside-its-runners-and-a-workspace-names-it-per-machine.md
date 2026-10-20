# Ruled 2026-10-20: a pool is kept beside its runners, and a workspace names it per machine

**Ruled by the operator on 2026-10-20**, on the questions #2679 raised. Items 1 and 2 are the
person's rulings, quoted. Items 3 to 6 are *decided in implementation, to be confirmed*
(D-2679-1 to D-2679-4). §2 and §8 are left as accepted, and this note adds to them.

1. **Pools are kept in `<config>/runners.json`, as a `pools` list beside `runners`** (the
   person's ruling): *"each `{"name","kind":"docker","image","cpus","memory","processes",
   "tmp"}`; limits left out fall back to the #2506 defaults; the file stays one per machine,
   `0600`, never committed; keep `version` and refuse a file of an unknown shape, as
   `runners.json` already does."* So the file stays at version 1, `pools` is left out while
   there is none, and a file with another top-level key, a pool row with another key or a value
   of another shape, or two pools of one name is refused whole (`purlis_core::runners`).
2. **Which pool a workspace's chats run on is set per machine and per workspace** (the
   person's ruling): *"in `machine.json`, keyed by workspace, set from the workspace's
   settings, never committed."* It is `chatsRunOn` in each remembered plane's entry,
   `{"<workspace>": "<pool>"}` (`purlis_core::machine::Store::set_chats_run_on`). §2's *"a
   project may name a runner, and never define one"* holds a step further: a workspace's pool is
   named on this machine alone, so a committed file never sends another person's chats to it.
3. **A setting reaches only a runner this machine paired and kept.** A workspace's pool starts
   a chat only where that pool and its runner are both kept in `runners.json`
   (`purlis_core::runners::runner_for`); any other name starts no chat and says which pool is
   missing, as §2 says of a runner. *Decided in implementation, to be confirmed (D-2679-1).*
4. **A pool has one runner, kept under the pool's name and of its image**, made, checked and
   paired as the pool is added, with its limits, under the lock of `runners.json`, so another add
   or a removal of it waits (`purlis_core::pools::add`). A failure takes back what the add made,
   the pool's record included; a run that fails after Docker made the container removes it and
   its volume. No other runner is kept under a pool's name. Removing a pool removes its runner whole first (the
   amendment of 2026-10-18 on removing a runner), then the pool. A pool that makes runners on
   demand is a cluster's (spec #2041). *Decided in implementation, to be confirmed (D-2679-2).*
5. **The limits are written as the person sets them**: `cpus` a number of CPUs to the
   thousandth, `memory` and `tmp` in MiB, `processes` a count, each left out where the default
   holds, so a default purlis changes later reaches every pool that did not set it. *Decided in
   implementation, to be confirmed (D-2679-3).*
6. **This desktop's link key is in the keyring as `purlis/@link-key/<device>`** (account
   `key`, §3 and §8's Keyring tier, named as the browser-logins key is), its private half as 64
   lower-case hex. It is made once, as a pool is added, under `link-key.lock` beside the machine
   store, and read back before a runner pins its public half; a link reads it and never makes
   one, so a key the keyring does not give is said rather than replaced (`purlis_core::linkkey`). *Decided in
   implementation, to be confirmed (D-2679-4).*
