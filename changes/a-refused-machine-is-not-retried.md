### Changed

- A pool's machine that answers but refuses (its sign-in, Docker or forwarding fails) is no longer retried on a timer. The pool's notice says it refused and that purlis stopped trying, and offers Try now (for a machine whose Docker is still starting after a reboot) and Check pool. Settings › Pools says "refused". A start on it is refused at once instead of waiting 30 seconds.
