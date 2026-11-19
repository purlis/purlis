### Changed

- A remote pool whose machine is up but whose Docker does not answer yet (after a reboot, `sshd` is up before `dockerd`) is retried on its own, with a growing pause, until Docker answers; its notice says "Docker on <host> isn't answering yet". Sign-in refused, forwarding refused and a full disk still stop the retries and offer Check pool. A runners file that cannot be read stops them too, and says why.
