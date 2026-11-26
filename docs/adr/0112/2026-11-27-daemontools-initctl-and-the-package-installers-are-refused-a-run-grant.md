# Amended 2026-11-27: daemontools, Upstart's initctl, aptitude, run-parts, capsh, cpanm and dnf are refused a Run grant

Decided under the operator's standing order to take the recommended path (#6557, #6559 and
#6564, left over by #6556), recorded here as delegated decisions. It follows *the service
managers, a link that passes through init.d … are refused a Run grant* (2026-11-13), which filed
these leads.

## Not read in source, and why each is refused all the same

The 2026-11-13 amendment checked each tool in its own source and refused none unread. This one
could not: from the sandbox that did it, `gh` reaches only `purlis/purlis` (a chat's forge token
is for its own repo), and a plain fetch of any other host, GitHub's raw files, Launchpad, Debian's
sources and cr.yp.to among them, was refused. No copy of any of these tools is on the machine.

Each tool below is refused whole, on `NOT_FOR_A_RUN_GRANT`, on what the tool is for, as its
manual describes it, and not on a line of its source. That is the conservative side: a refusal
only takes away an Allow on a run's notice, and a persona can still declare any of them as a tool
in a reviewed line. None is found harmless here, since that would need the reading this could
not do. Reading each in source, to cite its lines or to give an Allow back, is filed as a new
issue (see the pull request).

## daemontools (#6559)

- **`supervise <folder>`** changes to the folder and runs its `./run`, again whenever it exits.
- **`svscan [folder]`** starts a `supervise` on each folder in the one it is named (the current
  one by default), as runit's `runsvdir` and s6's `s6-svscan` do; those were refused on
  2026-11-13.
- **`multilog`**'s `!processor` action has a shell run the processor on each log file it
  finishes.
- Its chain loaders **`setuidgid`**, **`envuidgid`**, **`softlimit`**, **`setlock`**,
  **`pgrphack`** and **`fghack`** each run the program after them, as its `envdir` (refused
  since #5833) and s6's chain loaders (2026-11-13) do.

`svc`, `svok` and `svstat` are left offered: they talk to a `supervise` already running, over its
`supervise/control` and `supervise/status` files, and run nothing themselves.
