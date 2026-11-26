### Fixed

- A chat's forge listener and the hook channel give a connection's place back before the client sees the connection end, so a client that reconnects at once, with the listener at its limit, is no longer turned away.
