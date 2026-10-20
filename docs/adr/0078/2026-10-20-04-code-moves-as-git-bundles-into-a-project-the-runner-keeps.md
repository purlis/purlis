# Amended 2026-10-20: code moves as git bundles into a project the runner keeps

**Built by #2046** on §7 (RR-16) and §8. They are left as accepted, and this note says how code
first crosses to a pool's runner and back. The protocol's part is ADR 0068's amendment of the
same day. Item 1 was accepted by the steward on 2026-10-20 (D-2046-1). The rest is *decided in
implementation, to be confirmed* (D-2046-5 to D-2046-9).

1. **A git bundle, not a remote helper.** §7 named `git-remote-purlis` as git's standard way for
   git on the desktop to reach the link. A bundle is git's own file of a pack and the refs it
   carries, made by `git bundle create` and read by `git fetch` from the file. Each end makes and
   reads it in the host process that holds the link. A remote helper would be a second process,
   and it would need a way of its own into the window's link. `receive-pack` on the runner would
   run the receiving repository's hooks and read its config. *Accepted by the steward
   (D-2046-1).*
2. **Where a runner keeps a project.** The project the desktop holds on a runner is
   `<data>/projects/<project id>` there, in the runner's data home, and the repositories in it
   are where they are on the desktop: the project's own, and `workspaces/<workspace>/<repo>`.
   This replaces §8's `<data>/repos/<workspace>/<repo>.git`. A runner chat works in those
   folders as a local chat works in its clone (`purlis_core::codelink::repo_in`, which refuses
   any other folder and any folder reached through a link). *Decided in implementation, to be
   confirmed (D-2046-5).*
3. **What a runner takes** (`purlis_core::codelink::receive`). It takes only the refs the
   desktop named, each a branch (`refs/heads/…`) or a branch's snapshot of uncommitted work
   (`refs/purlis/snapshots/…`). Each name must be one git takes and none may be `HEAD`. A
   bundle that carries any other ref is refused whole. A repository the runner makes for it has
   no hook and starts on no branch the desktop sends. The desktop's `checkout` is checked out,
   on its branch and with its snapshot's uncommitted work, only where nothing is checked out yet.
   From then on, a branch one of the runner's work trees has checked out is the runner chat's
   own: the runner leaves it as it is, whatever the desktop sends of it, and the desktop takes
   it back. *Decided in implementation, to be confirmed (D-2046-6).*
6. **What a snapshot leaves out.** A snapshot of the project's own repository never carries
   purlis's state folder (its vaults and this machine's sessions) or the local settings file,
   ignored by the project or not (`purlis_core::codelink::plane_leaves_out`). A clone's snapshot
   carries its uncommitted work whole; what its own `.gitignore` ignores stays here. *Decided in
   implementation, to be confirmed (D-2046-9).*
4. **What the desktop takes back** (`purlis_core::codelink::take_back`). It takes the runner's
   branches as `refs/remotes/<pool>/<branch>` and nowhere else, and removes a branch from there
   once the runner no longer has it. No local branch, work tree or index of the desktop's is
   touched. *Decided in implementation, to be confirmed (D-2046-7).*
5. **How git reads a bundle, on both ends.** Every fetch of a bundle runs with no hooks
   (`core.hooksPath=/dev/null`), every object checked (`transfer.fsckObjects`,
   `fetch.fsckObjects`) and no submodule. No transport is allowed but the file one a bundle is
   read through (`protocol.allow=never`, `protocol.file.allow=always`), and there is no automatic
   garbage collection. That is on top of purlis's one git runner and its network entry. A
   snapshot of uncommitted work is staged in an index of its own, with no attributes from the
   work tree, so no filter it names runs (`purlis_core::codelink::snapshot`). The runner holds
   no forge credential: saves and pull requests happen on the desktop (FI3). *Decided in
   implementation, to be confirmed (D-2046-8).*
