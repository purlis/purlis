# Amended (2026-10-18, #2460): the Linux wrap starts with an environment purlis builds

Follows [purlis's own wrap on Linux](2026-10-16-purlis-s-own-wrap-on-linux.md) and
[a brokered run on Linux runs in purlis's own wrap](2026-10-17-a-brokered-run-on-linux-runs-in-purlis-s-own-wrap.md).

**Decided in implementation (#2460).** purlis's own wrap, `purlis _sandbox`, and the chat's side
it starts begin with an environment purlis builds, never the one the chat or the run is given.
That environment is carried for the program instead: each of its variables under a name of the
wrap's own, with one more that says how many. The wrap reads it back and gives the program
exactly that, and nothing of its own. Only the program it runs is given the chat's variables, so
whatever a library of purlis's own program reads from its environment, a chat never sets it.

- **Both starts.** A chat's terminal (in the window, and in a session host) and a brokered
  `secret exec` start the wrap this way. On macOS, `sandbox-exec` is the system's own program and
  is handed the environment as before.
- **Fail closed.** A wrap whose own environment holds anything else, or is missing a piece of
  what it carries, starts nothing and says so. What a terminal sets on every program it starts,
  its type (`TERM`) and its shell (`SHELL`), is let be beside it and not handed on.
- **Not on its command line.** The environment holds a run's credentials and a chat's hook
  token, and a process's arguments are readable by every user of the machine; its environment
  is not.
