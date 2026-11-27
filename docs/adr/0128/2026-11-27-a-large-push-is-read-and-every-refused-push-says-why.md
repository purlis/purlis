# Amendment 2026-11-27: a large push is read, and every refused push says why

Amends [ADR 0128](../0128-your-sign-in-brokered-one-approach-for-every-forge.md), D8.3 (the check of
a push's objects), and the amendment of 2026-11-23, "a refused push says why" (§1). From #7878.

## What was wrong

A push that changed `.github/workflows/ci.yml` ended in `send-pack: unexpected disconnect while
reading sideband packet`, `the remote end hung up unexpectedly` and `Everything up-to-date`. The
branch did not move, and no hold line or Ask was shown.

The push was larger than git's `http.postBuffer` (1 MiB by default). Before such a push, git sends
a **probe**: the same `POST …/git-receive-pack`, with a body of one flush packet (`0000`) and no
command. It does this to learn the push will be taken before it streams it. The route read no command
in the probe, refused it as `BodyUnreadable`, and, because a push whose commands do not read was
still answered `403` (§1 of 2026-11-23), git saw only an HTTP failure. Git never sent the push
itself, so the CI-file check never read it and no Ask could be raised. The refspec
(`<sha>:<branch>`) did not matter. The pack's size did: a small CI-file push was held as it should
be.

## Decision

1. **The probe is let through.** A `git-receive-pack` body that is exactly one flush packet names
   no ref and holds no object, so it moves nothing. The core lets it through as a push that makes
   no branch, and the listener sends it on without the CI-file check, because there is nothing in
   it to read. The push it announces comes next, and is checked and held like any other. Anything
   else with no command (two flushes, a flush and a pack) is still refused.
2. **A refused push whose refs cannot be repeated says why too.** Its commands do not read, or a
   ref name is not one a report can hold. It is answered `200` with the report-status
   `unpack <reason>` and no ref line, in the side-band its first command asks for. Git prints
   `error: remote unpack failed: <reason>` and exits non-zero. Only a push whose body was never read
   is still `403`, since without its body there is no way to know which side-band it asked for.
   §1's other rules are unchanged: the reason is one short line of purlis's own, nothing is
   forwarded, and a request with no credential is still challenged `401`.
3. **A chat whose route no longer takes its credential is told what still pushes.** A chat purlis
   put back after a restart keeps the sentinel of a route that went with the old app (D3a, as
   amended for #7694: only the app's own git gets a route for it). Its requests are refused
   `UnknownChat`, `WrongSentinel` or, after many, `Locked`. Git's reason and `gh`'s `403` now both
   say to push with `purlis worktree push` (the app's git) or start a new chat. D3a itself is
   unchanged: the running program is still handed nothing.

## Not changed

A push over the listener's push limit (32 MiB) is still answered `413` before its body is read.
Git prints it as `RPC failed; HTTP 413`. Answering it in git's words would mean reading a body the
limit exists not to read.
