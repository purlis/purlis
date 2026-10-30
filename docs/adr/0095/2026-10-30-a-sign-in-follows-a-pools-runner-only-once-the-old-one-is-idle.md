# Amended 2026-10-30: a sign-in follows a pool's new runner only once the old one is idle

**Built by #4011 and #4055**, under spec #4023. Item 1 is the person's ruling of 2026-10-30 (a
comment on #4055). Items 2 to 4 were decided in #4055 and are **decided in implementation, to be
confirmed** (D-4055-1 to D-4055-6 in the pull request). §3 (one sign-in per device, through the
harness's own flow) stands. This amendment does not copy a sign-in between devices: it moves a
sign-in from one runner of a pool to the runner that replaces it, on the same Docker host.

1. **One runner holds a sign-in at a time.** After an update, a Docker pool of purlis's own image
   gets a runner of the new version, and its old runner keeps its chats until they end
   (#2749). The old runner's sign-ins are carried to the new runner only once it has no chat
   left, just before it is removed with its home volume, which removes the old copy. Until then,
   a new chat on the new runner asks to sign in there.
2. **Only each harness's sign-in file is carried, and nothing is carried through this machine.**
   A short-lived container of the new image with no network, no capability and a read-only root
   copies the files on the Docker host. It reads the old home read-only.
3. **The carry writes nothing where a chat could have put something first.** The new runner
   serves chats while the old one drains, so the carry writes only into a folder of its own,
   which it makes under the runner's data home. It reaches that folder through no link, and it
   writes each file only where nothing is. The runner takes a carried sign-in in as it keeps the
   profile, for a profile it keeps for its desktop alone, and only the file of that profile's
   harness. A sign-in the profile already has on the runner stays.
4. **A carry that fails never holds the drain back.** It is said in the app's log, the old runner
   is removed all the same, and that profile is signed in again on the pool.
