# Amended (2026-10-17, #2342): a brokered run on Linux runs in purlis's own wrap

Follows [a vault's key names come from the app, and a value is never read in the chat](2026-10-14-07-a-vault-s-key-names-come-from-the-app-and-a-value.md)
and [purlis's own wrap on Linux](2026-10-16-purlis-s-own-wrap-on-linux.md).

**Decided in implementation (#2342).** A brokered `secret exec` for a sandboxed chat on Linux
runs the persona's declared tool inside purlis's own wrap, `purlis _sandbox`, under the profile
the chat's own sandbox was compiled to: the same profile text the macOS run hands `sandbox-exec`.
So the route a value takes in a sandboxed chat, `secret exec` through a declared tool, is the
same on Linux as on macOS, and the refusal of `secret get` and `secret cp` names it there too.

- **The wrap is the chat's own.** The program that wraps the run is purlis's own program as the
  chat's start checked and held it: never in a folder the chat may write, never one replaced
  since, and at the floor one the start held unwritten. The start records it for every
  sandboxed chat on Linux, a Claude Code chat's included; a Claude Code chat whose start could
  not hold one still starts, and a brokered run for it is refused.
- **Fail closed.** Without such a program, or on a kernel without the Landlock the wrap needs,
  the run is refused before any value is read, and nothing runs.
- **The run's own credential files come last.** A `--file` or `--dotenv` file, and a browser
  login's folder, are given back to the run after every denial of its profile, the vaults of
  every project included (2026-10-14-14), on both systems. Nothing else of the vaults class is.
- **Nothing the chat sets makes the wrap load code.** purlis's own process that applies the
  wrap starts from the run's environment, which it hands to the tool as `sandbox-exec` does, so
  the variables a brokered run never takes from the chat (D-PE-ruling-2: the dynamic loader's,
  an interpreter's start-up options) include the C library's, GLib's and GTK's module paths,
  which that program links.
