# Amended 2026-10-28: trusted app state lives in the app's data folder, outside the plane

**Built for #3718**, on §5 (the classes a chat's sandbox always denies). *The operator's ruling of
2026-10-28 (option a) decided where the records go; the rest was decided in implementation.*

1. **What purlis reads back about a project to decide something lives in the data home**, at
   `<data>/app-state/<clone key>/` (`purlis_core::appstate`), one folder per project clone by its
   clone key, `0700`. The plane's state folder keeps what a chat's own commands write there by
   design, and caches. The Skill studio's stores are the first moved: the drafts, the tries, the
   local grade record, the grades under way and their lock, the grade-after-edit opt-ins and their
   lock, the kept grade chats, and the records of the levels a sandboxed chat could have made.
   The other groups (persona and workspace pointers, approvals, the app's own records) follow, one
   pull request each.
2. **No data home, no store.** Where the machine has no data home the stores are unavailable and
   their callers refuse or say so; none falls back to the plane. A fenced build whose environment
   names no data home inside its fence keeps the folder in the project's own tree.
3. **The integrity class holds it.** A chat's sandbox denies writing `<data>/app-state` and
   everything below it, and reads it: a Try it's chat reads its laid-out skill there. Purlis's own Seatbelt
   wrap also holds `<data>/app-state` and `<data>` itself as entries, so neither is renamed, moved
   or replaced. Each is held under every spelling a case-folding disk takes. The class is
   `integrity`, not `human-powers`: these are purlis's own records, as they were in the plane's
   state folder, and the refusal a chat gets names purlis's own ways. Like every class, no grant
   reaches it, and a grant of a folder above it leaves it denied.
4. **Moved once.** At a project's first open after the upgrade, before any chat of it is put back,
   each store an older purlis kept in the plane's own state folder (under either spelling) is moved
   or removed, each only once all of it is carried, and `moved-from-the-plane` in the project's app
   state says so; nothing reads the plane's copies after that. Where a choice was open, the old
   copy is discarded rather than carried:
   - drafts and local grades are the person's own work, and are carried (a month's rows are added
     to the file of that month already there);
   - tries, grades under way and the locks are transient, and are removed;
   - the kept grade chats are a read-only view of chats that ended, and are removed;
   - the grade-after-edit opt-ins are a spend approval, and are removed: the person turns grading
     after each edit on again from a plan that shows the cost;
   - the records of the levels are vouching records, and are removed: every workspace and persona
     folder of the project then waits for the person's Accept in the Skill studio again.
5. **It is bound to its project, follows it, and fails closed.** A folder of the app state records
   the root it is for and the id of the project it belongs to, kept in the project's own `app/`; a
   folder of another project's at the same path is set aside and never read. A project its record
   of open chats says moved (V43) carries its drafts, local grades and kept grade chats from the old
   key's folder where that folder records the old root and the same project, never its opt-ins or
   levels records, and its workspace and persona folders wait for Accept again. A copy carries
   nothing. Where no app state can be kept, a sandboxed chat is not started; a start that could not
   be noted is held in the process instead, and records that cannot be read hold every folder. The
   move out of the plane reads only the state folder purlis was reading, and a state folder
   `$PURLIS_HOME` names may be shared by clones: its copies are left in place and read no more.
6. **No cost at a chat's open.** The move runs once per project open, and after its mark is written
   it looks at that one name alone. A chat's start reads the same stores it read before, in their
   new place.
