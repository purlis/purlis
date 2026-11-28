# Amended 2026-11-27: a keep sets the computer-use choice back to its default

Ruled by the person on 2026-11-27.

The amendment of the same day, which sets each project's MCP choices back to the defaults, leaves
every key beside `projects` as it is. One of those keys is also a choice a chat makes on the
runner: in Claude Code 2.1.332, `computerUseChoices` holds the computer-use choice of each
organisation and account (`"<organisation>:<account>"` to `"on"` or `"off"`), saved when its
first-use ask is answered. So **a keep takes `computerUseChoices` out of the runner's state file**,
at every keep, as it takes out each project's MCP choices. Claude Code's default holds none, and
with none it asks again. `ACCOUNT_CHOICES` in `crates/purlis-core/src/sandbox/claude_wrap.rs`
lists it. Every other key beside `projects` stays as it is.
