# Amended (2026-11-17): a pool whose machine doesn't answer can be forgotten here

§3 forgets a pool here, sending nothing there, when its pin no longer holds. A machine that is
gone, off, or off this network left the person with no way out at all: Remove needs the machine
to answer, and failed with Pool setup's words for why it didn't. The rest of this record stands.

## Remove that can't reach the machine offers Forget it here

Where Remove of a pool on another machine finds the machine can't be reached (its name doesn't
resolve, nothing answers on its port, or nothing was met through its jump host or proxy), nothing
is removed and the pool's row shows a Notice (cause key `pool-unreachable:<pool>`) with why, in
Pool setup's words, and **Forget it here**. Its words say what stays on that machine before the
person presses it: the runner's container and home volume, and that machine still listing this
one as the runner's peer, since the runner keeps its peers in its home.

Forget it here does what §3's forgetting does, and contacts nothing: the pool and its runners are
forgotten here, the workspaces and projects that ran their chats on it run them here again, the
window's links to its runner end, and the login opt-ins given on it go. It then says, by name,
the `docker container rm --force` and `docker volume rm --force` to run on that machine.

A machine that answers and refuses (a sign-in refused, no engine, an engine that changed) is not
offered it: what it says has a fix of its own, and forgetting would leave a runner that could
still be removed.

A cluster pool needs none of this. Its Remove already forgets it here when the cluster doesn't
answer, and says the `kubectl delete` that releases its claim.
