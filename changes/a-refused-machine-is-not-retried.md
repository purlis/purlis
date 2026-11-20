### Changed

- A pool's machine that answers but refuses (its sign-in, forwarding or a full disk fails, or its runners file cannot be read) is no longer retried on a timer. The pool's notice says it refused and that purlis stopped trying on its own, and offers Try now and Check pool. Settings › Pools says "refused". A start on it is refused at once instead of waiting 30 seconds.
- A pool whose machine is up but whose Docker does not answer yet (after a reboot, `sshd` is up before `dockerd`) is not refused: it stays unreachable and is retried on its own, with a growing pause, until Docker answers; its notice says "Docker on <host> isn't answering yet" and offers Try now.
