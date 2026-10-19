# Amended for #2692, every path reads the run record, 2026-10-20

Every path that tells a sandboxed chat from an unsandboxed one reads the record of how its run
started, in the direction that fails closed for that path, and a chat a runner composes by its
names is judged by the runner host's record. This note amends item 5 of the amendment for #2636
and adds to it; everything else stands. Item 1 takes session protocol 1.29, as the steward
assigned it; items 1 to 5 are **decided in implementation, to be confirmed** (D-2692-1 to
D-2692-5).

1. **A host says how it applied the sandbox to a run as it answers the run's start.** Session
   protocol 1.29, a minor: `program_started` carries `sandbox`, the same record `programs`
   answers, read by the host from what it started and never from the start. A runner's chat is
   started over a link between hosts, which may not ask for `programs`, so the start's answer is
   the one place the window hears it. The window keeps it for a chat its host composed by its
   names. A host before 1.29 says nothing, and its run is taken as sandboxed. *D-2692-1.*
2. **Each path reads the record in the direction that fails closed for it.** Where a sandboxed
   chat is allowed more (dispatching to another persona, a handoff's crossing, a dispatch while
   you are away, and the network record), a chat counts as sandboxed only where its run is
   recorded in a sandbox and the window holds what that sandbox compiled to. A run with no record,
   one recorded off, and one whose compile the window does not hold each count as unsandboxed.
   Where a chat with no sandbox is allowed more (a browser login, a browser's listing for a chat
   with no sandbox, the reach of a git action the app makes for a chat), a chat counts as
   unsandboxed only where its run is recorded off. *D-2692-2.*
3. **A git action reads the record as a path where no sandbox is allowed more.** The app's git
   actions let a chat with no sandbox clone and fetch from any host and skip the checks a
   sandboxed chat's cut is held to; the one thing they give a sandboxed chat alone, a commit the
   app makes for it, is nothing an unsandboxed chat cannot make with git itself. So a run with no
   record is held as sandboxed there, which is the closed direction for that path. *D-2692-3.*
4. **A browser login of a chat its runner composed in a sandbox is refused in words.** Its
   tools are the ones the runner armed, which the window keeps no record of, and a restart
   would not give it one, so it is not offered a restart. A runner chat recorded off has its
   persona's logins, as an unsandboxed chat on this machine does. *D-2692-4.*
5. **The vault-refusal Notice reads the record.** A chat is told of a vault it was refused
   unless its run is recorded off, whatever the window holds for it now. The Notice decides
   nothing, and its Allow is the person's. *D-2692-5.*
