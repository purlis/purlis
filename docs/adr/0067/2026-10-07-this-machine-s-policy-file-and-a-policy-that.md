# Amended (2026-10-07, #1343 and #1423): this machine's policy file, and a policy that requires the sandbox

This amends §4, and with it the last lines of §1 ("Policy locks values") and §7 ("Org policy can
forbid the opt-out").

1. **The machine layer of C9 is `/etc/purlis/policy.json`.** §4 names it
   `/etc/charter/policy.json`; no file of that name was ever read or deployed, and the product
   is purlis (ADR 0091), so the old path is not read (D-1343-2). It is the only layer read so
   far (D-1343-1): an MDM profile, a Windows policy key and an organisation's policy on a server
   stay with SD-14, and until one exists no policy is read on Windows.
2. **Its format is JSON, documented in `docs/plane-format.md`**, which is the reference for its
   keys: `owner` (who set it, as the window names them), and under `sandbox` the locks
   `presets`, `hosts`, `personal-hosts`, `persona-hosts`, `opt-out` and `write-grants`. Absent
   and `true` lock nothing. A key this version does not know refuses the whole file
   (D-1343-4), so a lock a newer purlis would keep is never dropped by an older one.
3. **It is trusted only where no one but an administrator could have written it**: a regular
   file, never a link, owned by root, in a folder owned by root, neither writable by anyone
   else, and no chat may write its folder (the human-powers class of §5). Nothing in the
   environment or in a project moves where it is read from. A file or folder that fails that,
   a file that does not parse, and one with an unknown key are **refused closed** (D-1343-3,
   D-1343-11): every lock is set but the presets. Access-control lists are not read
   (D-1423-5): on Linux one shows in the permission bits, and on macOS reading one needs a C
   call the codebase's no-`unsafe` rule does not allow, so an ACL root added there is an
   administrator's misconfiguration that this check does not catch. The folders above the
   policy's own are not checked.
4. **A policy that forbids the opt-out requires the sandbox** (the operator's ruling of
   2026-10-07, D-1423-1; it replaces D-1343-9). `"opt-out": false` means every chat on that
   machine runs sandboxed, in every project:
   - A project with no `[sandbox]`, or one that has not turned it on, runs every chat sandboxed
     there as if it had turned it on with the default presets. What its `[sandbox]` already says
     applies, and the policy's own `presets` and `hosts` hold it as they hold any project, so
     where the policy fixes the presets, those are the most it reaches. Nothing is written to
     the project: the requirement is the machine's, and a teammate without the policy is not
     held to it. The one-time offer to turn the sandbox on is not shown there.
   - Settings shows the mode as "On, required by policy, set by <owner> in <file>", with no
     control.
   - A system with no sandbox backend (Windows today) **refuses** a harness chat. It is never
     started unconfined, and purlis is never the actor of a `trust.sandbox.off` there. The
     refusal names the policy and its owner. Ruling V21 3 (Windows starts at the opt-out, with
     purlis as the actor) stands only where no policy requires the sandbox.
   - No refusal sends the person to the opt-out: each names the policy and its owner instead,
     and leads with the policy ("policy requires the sandbox for every chat on this machine"),
     not with the project (#1431).
   - §1's "absent is not off" stands for the project's own file. A policy is the one thing that
     turns the sandbox on where the project did not, and only on the machines it is on.
5. **What a lock drops is said where it is kept or added** (#1423): Settings refuses a host
   the policy does not allow when it is added, with the policy's sentence, by its Add and by
   Edit as TOML alike, while one the file already held is kept (#1431); the Granted list marks a
   kept grant the policy now drops as not in force, and still offers Revoke on it (#1431); and
   the one-time Notice of a change to the project's hosts names only the hosts a chat reaches.
