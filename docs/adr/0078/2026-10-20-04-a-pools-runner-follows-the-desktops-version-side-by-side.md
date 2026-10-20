# Ruled 2026-10-20: a pool's runner follows the desktop's version, side by side

**Ruled by the person on 2026-10-20**, on the questions #2749 raised; **built by #2749**. It
delivers §5's *"versions side by side, and the old one drains"* (and #723) for a Docker pool.
Items 1 and 2 are the person's rulings. Item 2 amends item 4 of the amendment of 2026-10-20 for
#2679 (D-2679-2: *"a pool has one runner … No other runner is kept under a pool's name"*). Items
3 to 7 are *decided in implementation, to be confirmed* (D-2749-1 to D-2749-5).

1. **The runner image's digest is in the signed update manifest** (the person's ruling).
   Release builds the runner image with the app's version stamp, and `latest.json` (`dev.json`
   on dev) gains `runnerImage`, trusted as ADR 0042 trusts the manifest. `release.yml` calls
   `runner-image.yml` for the commit and the version it publishes, and each binary of the
   release, the image's `purlis` with them, says that version (`PURLIS_VERSION_STAMP`). The
   image no longer publishes on its own green `main`.
2. **A pool's runner is drained side by side** (the person's ruling). Runner rows in
   `runners.json` gain `"of": "<pool>"` and `"draining": true` (plane-format.md). A pool's new
   chats start on its one runner of its image that is not draining; a draining runner keeps its
   chats and is removed, container and home, once no chat is left on it.
3. **The manifest's line is signed by the updater's key.** The manifest is fetched over TLS and
   is not itself signed, so `runnerImage` is `{"image", "signature"}`: a minisign signature by
   the key the app ships, over the image's exact line, with the release's version in its trusted
   comment, made in the `release` environment by `release-manifest`'s `sign-runner-image`.
   `release-manifest` refuses a release, and the app a manifest, whose line does not verify, is
   signed for another version, is not named by its digest, or is not
   `ghcr.io/purlis/purlis-runner`. *Decided in implementation, to be confirmed* (D-2749-1).
4. **The app reads the manifest of its own version.** A while after each launch and at every
   updater look, the window asks its channel's manifest through the updater, comparing for the
   same version rather than a newer one. A manifest of another version names no image, and
   nothing moves until the app is on the version the channel serves. *Decided in
   implementation, to be confirmed* (D-2749-2).
5. **The new runner is made, checked and paired as a pool's runner is**, beside the old one,
   called `<pool>-<the first 8 hex digits of its digest>`, held to the pool's limits; only then
   is it kept, the old one marked draining and the pool's image moved, under the lock of
   `runners.json`. What fails takes back the new runner and leaves the pool as it was; the next
   look tries again. A runner's name is at most 49 characters, a pool's 40. *Decided in
   implementation, to be confirmed* (D-2749-3).
6. **A draining runner is removed once a link to it lists no chat that has not ended**,
   unpaired first over that link. One that does not answer is not known to be idle and stays
   until a look finds it so. Removing a pool removes each of its runners, the draining ones
   first. *Decided in implementation, to be confirmed* (D-2749-4).
7. **A mismatched pair is always said.** Every link the desktop's host makes to a pool's runner
   first reads the runner's `purlis --version` over `docker exec` and logs a line where it is not
   the desktop's; the link is still made, since a draining runner of the older version keeps
   serving its chats. The Pools group says, for each pool whose runner runs another version, what
   both run, and for a pool of purlis's own image that new chats move once this version's image
   is published; for a pool of a team's own image, to build it again from this version and add
   the pool again. A team's own image is never moved. *Decided in implementation, to be
   confirmed* (D-2749-5).
