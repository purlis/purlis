# Amended 2026-10-27: a profile is kept, and signed in, on each runner

**Built by #2049**, under spec #2041 (*"Profiles on runners: … Subscription profiles are signed in
once per device through the harness's own flow, offered when a pool is added and on a move; their
sign-in lives on a kept claim's volume. A profile pins its harness version and the runner installs
exactly it."*). Items 2, 3 and 7 apply the dispatcher's rulings as steward of 2026-10-27 (and item
2 the ruling of 2026-10-26 on D-2050-4). The rest was decided in #2049 and is **decided in
implementation, to be confirmed** (D-2049-1 to D-2049-9 in the pull request). §3 (one sign-in per
device, through the harness's own flow) stands, and is what this applies to runners.

1. **A runner keeps a profile for each of its desktop's profiles.** The desktop sends its runner
   the profile's name, its harness, whether it is for sandboxed chats only and the version it is
   pinned to (session protocol 1.43, `keep_profile`, on the desktop's own link to the runner;
   never relayed). **No file of the desktop's profile folder, no setting, no sign-in and no vault
   reference has a field in what is sent**, so none can reach a runner through it. The runner
   marks every record it keeps so with the desktop's device id (`kept_for`).
   - A profile purlis made on the desktop is made on the runner in a folder of the runner's own,
     in its own data home, with nothing imported and no sign-in.
   - **The desktop's folder adopted in place (`claude`, `codex`, `opencode`) is the runner's own
     default folder of that harness**, in the runner's home, adopted in place there: a chat on it
     runs as the runner's built-in did, so a runner already signed in stays signed in and its
     conversations resume. Only such a record, kept for a desktop, is signed in by purlis; a
     folder a person adopted on their own machine never is.
2. **Only a runner shown to be the person's own is sent a profile or signed in**: a Docker pool
   whose engine is a local socket on this machine, by the check browser logins and tools use,
   until a pool can be marked owned (#2561). To any other pool nothing is sent, and its chats
   start on the profiles its runner has of its own; a runner of a purlis before 1.43 is sent
   nothing either. Signing in on a pool is offered for an owned pool alone.
3. **A subscription is signed in on the runner through its harness's own flow, once per device.**
   Sign in opens a shell tab on the desktop and types the line that runs the runner's own
   `purlis profile sign-in` there, through the desktop's `docker` with **Docker settings of
   purlis's own** (an empty configuration, plugin hooks off, no context; none of the person's
   Docker settings is read), on the engine the check showed to be the person's own, in the
   container by its id, checked as it was made first. The person drives the harness's flow in
   that tab; the harness keeps the result on the runner. purlis reads none of it, and nothing of
   the desktop's sign-in goes to the runner.
4. **Adding a pool ends with its profiles**, each with Sign in where it is not signed in there;
   the profile page says where each profile is signed in (this Mac, and each pool).
5. **A start on a pool keeps its profile there first, and asks the runner whether it is signed
   in.** Signed in, or where its harness cannot tell, it starts with nobody asked: a moved chat on
   a signed-in profile starts silently. Signed out, it starts nothing and waits as one Ask,
   *Profile <profile> on pool <pool>*, whose *Sign in on pool <pool>* signs it in there; the Ask
   leaves once the runner says it is signed in. What a runner says in refusing is shown as its
   own words, cut short.
6. **Only a profile signed in by a subscription goes to a runner in this version.** A key's and a
   cloud's credentials are handed to no chat yet (#2151), on this machine or a runner; a local
   model is on the desktop, which a runner does not reach. Each is refused in words, and nothing
   of it is sent (#3447).
7. **The runner holds the pin the desktop sends**, and clears it when the desktop's profile has
   none. A runner chat starts on the pinned version or a newer one of the same major, as on the
   desktop. A desktop's adopted folder follows its harness and carries no pin on the runner.
   Installing exactly the pinned version on the runner is #3446.
