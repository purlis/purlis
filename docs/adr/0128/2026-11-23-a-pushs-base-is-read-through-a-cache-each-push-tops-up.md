# Amendment 2026-11-23: a push's base is read through a cache each push tops up

Amends [ADR 0128](../0128-your-sign-in-brokered-one-approach-for-every-forge.md), D8.3 (a push's
objects are read in a quarantine), and the amendment of the same day, "a thin pack on an
integration branch is taken in". P0, dev.2408.

## What was wrong

Every `git push` from a chat through the one-login forge route to a large repo was refused, even a
new branch at an existing commit with no new objects: `purlis did not send this push: it could not
read the base (git could not fetch the base from the forge).` The quarantine fetched the base and
its whole history into an empty repository on every push, through `git::run_network`, which holds
a call to 120 s. On a large repo the fetch took longer; git was killed with nothing on stderr, and
the refusal's text was a fallback that named no cause. The advertised tips (the same day's earlier
amendment) cost the same.

## Decision

- **A cache per forge repo.** `purlis_core::heldact::ObjectCache`: a repository of the app's own, one
  per (host, repo), in `object-cache/<hash>/` of the app's data home on the machine (not the route
  folder, which is RAM-backed or purged on Linux and macOS), which no chat can read or write: the
  sandbox denies the folder to every chat, under every spelling of the data home (tier in
  `docs/plane-format.md`: Machine, device-bound, rebuildable). The base, and the advertised tips
  where a pack needs them, are fetched **into the cache**, whose private refs tell the forge what it
  holds, so each push is sent only what is new.
- **The quarantine reads through it as an alternate** (`objects/info/alternates`), so its own store
  holds only the pushed pack. A full fetch, not `--filter=blob:none`: the scan reads the blobs of the
  diff.
- **Why an alternate is safe.** An object is its hash. What a push is read for is decided by the
  walk from the pushed tip down to the base (`rev-list --objects <tip> --not <base>`), which reads
  and scans every object it reaches wherever the object is stored; a quarantine's store and an
  alternate's are the same store to git. The cache may hold more than the base does, and a pack
  that leaves out an object the cache holds is now complete where it was once fetched for. That
  object is what the forge sent and git hashed on the way in, and the walk reads it when the tip
  reaches it. Every object reachable from the pushed tips and not from the base is still scanned.
  `index-pack --strict` still hashes and checks every object of the chat's pack, and a tip whose
  objects are not all there is still refused.
- **The first fill is never on the connection.** The listener answers a chat's push inside a
  connection total of 10 minutes, and a verdict written after it is lost: a Forward given after 10
  minutes could deliver the push and then fail to answer, so the chat would see a failed push that
  had landed. So the push that finds the cache empty starts the fill on a thread of the app's own,
  owned by the cache (`objectcache::FILL`, 15 minutes, `git::run_network_within`), and is **refused
  at once**: "purlis is still reading <repo> from the forge for the first time; try the push again
  in a few minutes" ("again", with the last failure, after a failed fill or a cache made again).
  Every push that arrives while it runs is refused the same way. Nobody waits for the fill; a
  failed fill leaves the cache not filled, so the next push starts it again.
- **One bound for all of a push's fetches:** `objectcache::PUSH`, 240 s, in all (the base, the
  default branch's tip, the listing of the forge's branches and the tips), well inside the 10
  minutes, so every refusal's text reaches the chat. A top-up that outruns it is refused with that
  reason ("could not finish reading ... within 240 s, and is reading on in the background") and
  finished on the cache's thread, so the next push finds it done. A push does not wait for another
  push's top-up longer than its own bound, and waits without parking a thread on a lock.
- **Concurrency.** One lock per cache in the app (the app is one per user), and git's own locks.
  A later top-up is short, and a push waits for the one in progress, within its bound.
- **Size and cleanup.** `git gc --auto` after an update; `tmp_pack_*` files a killed fetch left are
  removed before the next one. The base refs kept are the 32 **last used**, never the commit date's
  newest, and the base just fetched is always among them; the tip refs are the forge's advertised
  tips of the last run. A cache past 10 GiB is made again, but only when no quarantine holds a
  lease on it, since a held push reads through it until its ask ends. A repo whose history alone
  is past 10 GiB is **refused with that reason and kept**, not deleted and fetched again on every
  push.
- **Self-repair.** A cache whose folder git does not take for a repository (an empty `.git`, a
  missing `HEAD`) is made again when no quarantine other than the one being opened reads it, and
  filled again on the cache's thread; while one does, the push is refused with the reason.
- **Fail closed, with the real reason.** A git stopped by its deadline now leads its stderr with
  `git took longer than N s and was stopped`, so a refusal says that and not "could not fetch". Any
  other doubt (the cache cannot be made, a fetch fails) refuses the push as before.

## Consequences

The first push to a large repo after install is refused at once with the "first time" text while
the cache fills in the background; the retry, a few minutes on, succeeds. The cache survives
reboots and app runs, since it is in the data home and not in a temporary folder. A cache is never trusted for what it is not: it is a copy of what
the forge sent, and a missing or damaged one is rebuilt by the next fetch. The growth limit on the
forge's tips (512 MiB per run) now counts what the run added to the cache.
