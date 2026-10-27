# Amended 2026-10-27: a profile is kept, and signed in, on each runner

**Built by #2049**, under spec #2041 (*"Profiles on runners: … Subscription profiles are signed in
once per device through the harness's own flow, offered when a pool is added and on a move; their
sign-in lives on a kept claim's volume. A profile pins its harness version and the runner installs
exactly it."*). Item 2 applies the dispatcher's ruling of 2026-10-26 on D-2050-4. The rest was
decided in #2049 and is **decided in implementation, to be confirmed** (D-2049-1 to D-2049-9 in
the pull request). §3 (one sign-in per device, through the harness's own flow) stands, and is
what this applies to runners.

1. **A runner keeps a profile of its own for each of its desktop's profiles.** The desktop sends
   its runner the profile's name, its harness, whether it is for sandboxed chats only and the
   version it is pinned to (session protocol 1.43, `keep_profile`, on the desktop's own link to
   the runner; never relayed). The runner makes the profile in its own store, in a folder of its
   own in its own data home, which on a pool is the runner's home volume, with nothing imported
   and no sign-in. **No file of the desktop's profile folder, no setting, no sign-in and no vault
   reference has a field in what is sent**, so none can reach a runner through it. The desktop's
   folder adopted in place as `claude` is kept on the runner under that name, in a folder the
   runner made, and stands there for the built-in of that name.
2. **Only a runner shown to be the person's own keeps a profile and is signed in**: a Docker pool
   whose engine is a local socket on this machine, by the check browser logins and tools use,
   until a pool can be marked owned (#2561). Every other pool is refused in words and sent
   nothing.
3. **A subscription is signed in on the runner through its harness's own flow, once per device.**
   Sign in opens a shell tab on the desktop and types the line that runs the runner's own
   `purlis profile sign-in` there, through the desktop's `docker`, on the engine the check showed
   to be the person's own, in the container by its id, checked as it was made first. The person
   drives the harness's flow in that tab; the harness keeps the result in the runner's folder.
   purlis reads none of it, and nothing of the desktop's sign-in goes to the runner.
4. **Adding a pool ends with its profiles**, each with Sign in where it is not signed in there;
   the profile page says where each profile is signed in (this Mac, and each pool).
5. **A start on a pool keeps its profile there first, and asks the runner whether it is signed
   in.** Signed in, or where its harness cannot tell, it starts with nobody asked: a moved chat on
   a signed-in profile starts silently. Signed out, it starts nothing and waits as one Ask, *Sign
   in <profile> on pool <pool>*, which leaves once the runner says it is signed in there.
6. **Only a profile signed in by a subscription goes to a runner in this version.** A key's and a
   cloud's credentials are handed to no chat yet (#2151), on this machine or a runner; a local
   model is on the desktop, which a runner does not reach. Each is refused in words, and nothing
   of it is sent.
7. **The runner holds the pin the desktop sends.** A runner chat starts on the pinned version or
   a newer one of the same major, as on the desktop; installing exactly that version on the
   runner is #3446.
