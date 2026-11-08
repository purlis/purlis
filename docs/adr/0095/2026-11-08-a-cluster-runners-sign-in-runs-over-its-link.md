# Amended 2026-11-08: a cluster runner's sign-in runs over its link, and Open in terminal types purlis

**Ruled by the person on 2026-11-08.** A cluster runner's sign-in was run through this machine's
own `kubectl exec` into the pod its pairing named, which reaches the pod of that name. The link to
the runner proves it by the key pinned at its pairing. A sign-in now goes where the link goes.

1. **The window signs a cluster runner's profile in over its own link to it.** Session protocol
   1.79's `start_sign_in` has the runner run its own `purlis profile sign-in` with pipes, as the
   exec ran it, held to the link it was asked on: `sign_in_next` reads what it printed,
   `sign_in_input` types to it and `end_sign_in` or the link's end ends it. It is the paired
   desktop's alone, on its own link, granted and refused exactly as `sign_in_redirect` is, and the
   runner runs it only for a profile it keeps for that desktop. What it prints is read for its
   steps exactly as before.
2. **Where no link carries it, the exec is held to the pod judged.** A runner before 1.79, and
   `purlis profile sign-in --pool` in the person's own terminal, which holds no link, still sign in
   through `kubectl exec`. The uid of the pod of the runner's name is read right before the exec
   and again once the exec has reached the pod, on the first thing the sign-in prints, before any
   of it is shown or anything is passed to it. Pods of one name never share a uid, so the same uid
   on both sides proves the exec reached the pod judged. Otherwise the sign-in is stopped and none
   of it counts. Its status check over exec is held the same way.
3. **Open in terminal types this machine's own `purlis`.** The line is `purlis profile sign-in
   <name> --pool <pool> --in-terminal`, with `--provider` and `--way device` where they apply. It
   passes the gate the window's sign-in passes each time it runs, a line run again from a shell's
   history too, then runs the runner's own sign-in in that terminal. It asks the running window
   first, as the command's driven sign-in does: the window answers the line it just typed with the
   click that typed it, once, within a minute, and asks the person of any other run. A sign-in held
   as in 2 is never typed into a shell as the runner's own line.
