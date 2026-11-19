# Which of purlis's refusals run: `[policy]`

purlis can refuse a chat many things: a branch move in the plane root, an unattended publish, a
merge, a commit with a credential in it, a read of a vault. Which of these run is the project's
choice, in its committed settings file (`purlis.toml`, or `charter.toml` on a project that still
has that name), under `[policy]` (rulings Q308, Q308a and Q308b;
[ADR 0125](adr/0125-blocks-are-off-by-default-and-opt-in-per-family.md)).

```toml
[policy]
template = "team"          # open, watch, hold, team or strict; watch when absent
guards.release = "log"     # one family, apart from the template
```

## Modes

- **`off`**: the check does not run.
- **`log`**: the check runs and allows. Nobody is shown anything; one quiet line in the chat's
  enforcement log (`.purlis/app/enforcement/<chat>.jsonl`) names the rule, the family, the chat
  and the host, never the command, a path or a value.
- **`enforce`**: the refusal, as purlis has always made it.

## Templates

| Template | What runs | Sandbox (wired in a later release; today `[sandbox]` decides) |
|---|---|---|
| `open` | nothing | none |
| `watch` (the default) | everything logs; `secrets`, `vaults` and `guards.credential` are enforced | the floor |
| `hold` | `watch`, and `guards.merge` enforced | the floor |
| `team` | `guards.merge`, `guards.release`, `guards.save`, `guards.credential`, `secrets`, `vaults` and the families Q308b added enforced; the rest log | the full sandbox |
| `strict` | everything enforced: purlis before Q308 | as `[sandbox]` says |

The families, and what each covers, are listed in
[`docs/plane-format.md`](plane-format.md#policy--which-of-purliss-refusals-run). In this release
the git, merge, release, save and credential guards, `secrets`, `dispatch` (a pair
nothing covers starts with no grant) and `asks` (a host is answered Allow) switch; every other
family is a key already and stays enforced until it is wired. One key decides a sandbox rule: `secrets.credential_files = "enforce"` holds your SSH keys and
forge and cloud credential files from chats at the floor too, as `team` and `strict` do; the full
sandbox always holds them (ADR 0067 §5 class 6, Q325). Your own denies hold in every mode,
`open` included: a dispatch never and a host you kept blocked.

## Who can change it

- **Only a committed change counts**: the file is read as it stands at the plane root's `HEAD`.
  Where git cannot read `HEAD` or the file's content in time, the plane's `.git` does not read,
  or the repository has no commit yet, the project runs `strict`, never the working tree's file.
  That is not kept as what you accepted: once git reads it again, the project runs as before.
- **A tightening applies at once**, to every chat, from its next tool call.
- **A loosening waits for your press.** The window names each family the project now runs looser
  than you accepted, with **Accept**, which accepts exactly what it showed you: if the committed
  file, your local file or an administrator's lock changed since, nothing is accepted. Until you
  press it, your chats keep what you accepted before. What you accepted is kept on this machine,
  sealed by purlis's app; a chat can neither press Accept nor write it.
- **Only the app decides.** A hook or a command a chat runs asks the app, and refuses as `strict`
  does where the app does not answer.
- **A chat cannot change `[policy]`.** Its write of the settings file is refused, and so is its
  commit of a change to the table. A branch folder's copy of the file is never read.
- **Your local settings file can only make a family stricter.**
- **An administrator's lock wins.** `/etc/purlis/policy.json`'s `enforcement` list names families
  that are always enforced on that machine, and its other locks keep their own families enforced.
- **A value purlis cannot read never loosens anything**: an unknown template is `strict`, an
  unknown mode is `enforce`, and the window says which.

## When a guard is wrong about your case

Set its family to `log` in `[policy]`, commit it, and press Accept in the window; the chat's
next start runs it that way. Or run the command yourself, in your own terminal: these guards
bound what an agent does with your authority, never what you do.

## What an administrator's forbid does, and what is never asked

- **An administrator's forbid is always enforced**, whatever the project's template or family
  mode: every Organisation-level forbid in `/etc/purlis/policy.cedar` and every one `policy.json`
  translates to is `enforce` unless the administrator's own `[policy.rules]` says otherwise. A
  project can't loosen it by a mode, a rule mode or a rule of the same `@id`. (A forbid the
  administrator writes there with `@level("project")` is project-level by design, and a project's
  rule of the same `@id` can replace it.)
- **A request's host is a name or an address alone**, lower case, with no path, no port (the
  port is the request's `context.port`) and no wildcard. A host that is not one, such as
  `localhost`, a single-label name, a loopback, link-local or cloud-metadata address, an IPv6
  address without its brackets, or `evil.com/x.corp.example`, is **unasked**: it is refused in
  every mode, `off` and `log` included, because no rule can be put on it. The members of the sets
  `known-hosts` and `production-hosts` are read the same way, and a member that is not such a
  host (or carries a port, or starts with `*.`) makes the file broken. A set can't be named
  `*.something`: that is the name the policy gives the hosts under a domain.
- **Hazard for administrators: one such member breaks the whole Organisation.** A
  `known-hosts = ["localhost"]` (or any single-label name, a port, a wildcard) in
  `/etc/purlis/policy.toml` makes the Organisation's file broken, and a broken Organisation reads
  as `strict` plus every lock under the full sandbox, not as that one set left out. Check the
  window's Notice after editing it.

## Known gaps: what a harness cannot enforce

A rule in force (a family whose mode is not `off`) that a harness cannot enforce on this system is a
**known gap**. purlis declares it and the chat still starts: in Settings under *Sandbox › Known
gaps*, in `purlis doctor` (wherever any family is not `off`, which the default `watch` template
already is for `secrets`, `vaults` and `guards.credential`), in the chat's start event and on its
pane, for a chat started from the window as for one a host starts. A family set to `off` declares
none. Nothing is declared as held where it is not: a chat whose sandbox is lifted (the person's
opt-out, or a floor that could not be applied) has a gap for every family the sandbox, the proxy or
an OpenCode shim's protected files would hold, and a declared harness with no adapter has every
family as a gap.

`context.gaps` on `start` holds the keys of the families that are `enforce` and have a gap for the
chat being started. A project that wants such a chat refused writes the rule itself; the shipped
default has none:

```cedar
@id("trust-no-start-on-a-sandbox-gap") @level("project")
forbid (principal, action == Action::"start", resource)
  when { context.gaps.contains("sandbox") };
```

**The rule refuses only where the family `trust` is `enforce`.** Under the default `watch` and
`hold` templates `trust` is `log`, so the rule above refuses nothing there: the chat starts, and
its tab says once that the rule would have refused it. Write `trust = "enforce"` under `[policy]`
(the `team` and `strict` templates do) for the refusal. Mind what the rule says: `contains("sandbox")`
is true of every start once `trust` is enforced, because Claude Code's reads are a gap on every
system (`claude-code-reads`), and a lifted sandbox is one too; such a rule refuses every start.

An `@ask` forbid on `start` refuses too, until asking at a start is wired (slice 36): the refusal
says the rule asks and that asking at a start is not wired yet. Write a plain forbid.

The refusal names the rule and the gaps. `crates/purlis-core/src/knowngaps.rs` holds the capability
table (each action's enforcement point per harness, system and sandbox, or the gap that stands in its
place) and every gap's entry; `context.machine` is unset on every system (it is sealed until the
person confirms it), so a rule reading it is denied.
