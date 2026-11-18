# Amendment 2026-11-17: the sandbox level, network, ports and asks switch

Amends [ADR 0125](../0125-blocks-are-off-by-default-and-opt-in-per-family.md) with items 1, 3 and
5 of the Q308 follow-up, and the person's ruling **Q328** of 2026-11-17.

## The sandbox level is one switch

- The project's level is the `sandbox` family: `[sandbox] mode` where the committed file names
  one (`on` the full sandbox, `off` no sandbox under `policy = "local"`, the team's off), else
  the template's (`open` off, `watch` and `hold` the floor, `team` full, `strict` the floor, as
  before Q308). There is no `sandbox` key in `[policy]`, so never two switches.
- It is resolved with the families, kept in the sealed acceptance as `sandbox = off | floor |
  full` (an acceptance from before, `enforce`, reads as the floor), and in force at the stricter
  of the committed level and the accepted one: **a tightening applies at once, a loosening waits
  for the press**. That includes taking `mode = "on"` out.
- **The team's off is the sandbox family's press.** Its Accept is an acceptance of the level
  `off`, Keep it on for me keeps at least the floor, and the policy press on a team's off answers
  its ask, so nobody is asked twice.
- The team's-off answer is kept unsealed, so its Accept lowers the sealed level only for an off
  the project committed and still says (`team::team_off` is `Accepted`). An acceptance that is
  broken runs every family enforced at the full sandbox, never the floor.
- The level is the default of the team's bound (`team::allowed_levels`), so every start reads it
  through the one seam: `decide_granted`, `for_start_at`, a person's own level, the chain's
  strictest. A start that names no way it was made still never runs without a sandbox.
- A process that is not the app never runs a level looser than the file alone says.

## Network

`network` off or logged: the egress proxy carries a host nothing lists, which a person could
allow, with nobody asked, and logs each host and port once under `log`. This machine, a
link-local or metadata address, a host an administrator pins as never allowed and the person's
Keep blocked stay refused: Keep blocked is checked before any lift, since it holds in every mode
(Q308b). A confined extension's proxy is not touched. A runner applies its own locks over what a
desktop relays: a family its administrator holds enforced is never lifted there.

## Ports (Q328)

- **`ports` takes off or enforce.** The kernel tells nothing of a connection it let through, so
  there is no line to write: `log` is enforced, and said, as `vaults`'s is. Every template but
  `open` enforces it.
- **`off`** (a project's pressed `ports = "off"`, or `open`) lifts the floor's and the full
  profile's holds on local sockets and this Mac's loopback ports for a wrapped harness. purlis's
  own stay denied after the lift: its ports as the chat was compiled, and its sockets wherever
  they are (every project's state folder, every hook socket, the session host's and its window's,
  every chat's server sockets, the forwards to a pool's engine), with the chat's own hook socket
  given back after them. Another program's socket is reached, a container engine's
  (`docker.sock`) among them: only under a `ports = "off"` the person pressed. **Declared
  residual:** a loopback port purlis binds later (another chat's proxy, a door) is not known at
  compile time and is reachable.
- Claude Code's sandbox is unchanged under every policy: **`allowAllUnixSockets` and
  `allowLocalBinding` stay `false`**, since each opens every socket or loopback port at once and
  nothing after it can hold purlis's own back. Its chats reach a socket a person allowed by its
  path, as before.
- A `ports = "log"` makes `ports` enforced, and is said in the Notice; the rest of the project's
  policy stands (it is read, not refused).
- Linux's sandbox is unchanged: it reaches no socket under any policy.
- Why not under `log`: a socket or port may be a container engine's or a cluster's, whose
  containers read the vaults from outside the wrap, the same escape the floor's process rules
  are kept for.

## Asks (Q328)

- A **folder** ask is answered Allow for the chat under `asks` off or log, through the very grant
  the Notice's Allow makes: judged by the core, so a denial class, a folder off the allowlist and
  a lock still refuse and raise the Notice. It is audited, logged under `log` with its path, and
  taken at the chat's restart. **Only a folder of the project, or one the person listed, is
  answered so**: never a held folder a later program loads code from (the folders a person may
  lift for one chat, `~/.config/gh`, fish's `vendor_conf.d`, nvim's `site/plugin`, …), nor a
  folder the floor holds on `PATH`. Those raise the Notice in every mode.
- A **socket** ask is answered so only under `asks = "off"`; under `log` it still asks.
- A host ask a session host's board answers, and a host its proxy carries for `network`, are
  told to the window (session protocol 1.93, `Confine::lifted`, `SandboxTold::Lifted`), which
  alone resolves the policy and knows the chat, and logs it.

## Reported as not in force

Without a wrap, `network`, `ports` and `processes` mean nothing: the press that would take the
level to `off` says so.
