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

| Template | What runs | Sandbox (a committed `[sandbox] mode` overrides it) |
|---|---|---|
| `open` | nothing | none |
| `watch` (the default) | everything logs; `secrets`, `vaults`, `guards.credential` and `trust` are enforced | the floor |
| `hold` | `watch`, and `guards.merge` enforced | the floor |
| `team` | `guards.merge`, `guards.release`, `guards.save`, `guards.credential`, `secrets`, `vaults` and the families Q308b added enforced; the rest log | the full sandbox |
| `strict` | everything enforced: purlis before Q308 | as `[sandbox]` says |

The families, and what each covers, are listed in
[`docs/plane-format.md`](plane-format.md#policy--which-of-purliss-refusals-run). In this release
the git, merge, release, save and credential guards, `secrets`, the sandbox level, `network`,
`ports` (off or enforce only), `dispatch` (a pair nothing covers starts with no grant) and `asks`
(a host and a folder are answered Allow; a socket only under `off`) switch, and so do the nine
families ruling Q308b added (`guards.forge`, `guards.shape`, `guards.plane`, `mode`, `tools`,
`trust`, `runners`, `consents` and `limits`; under `limits = "log"` a limit is counted, not
applied, up to an administrator's ceiling where there is one); every other family is a key already
and stays enforced until it is wired. Which choke point switches each, and the family of every
decision purlis makes about a chat, are in
[`docs/policy-inventory-families.md`](policy-inventory-families.md). One key decides a sandbox
rule: `secrets.credential_files = "enforce"` holds your SSH keys and forge and cloud credential
files from chats at the floor too, as `team` and `strict` do; the full sandbox always holds them
(ADR 0067 §5 class 6, Q325). Your own denies hold in every mode, `open` included: a dispatch never,
a host you kept blocked, and your harness's ask and deny rules for a program, behind which a Bash
guard that fails to answer refuses the call. Where the sandbox is on, a Bash guard that fails to
answer refuses the call too, since it judges the sandbox's own rules (Q327).

**The names of purlis's own sockets are held at every floor, whatever `ports` says.** A chat in
the sandbox can neither move nor link `hooks.sock`, `slots.sock`, `purlisd*.sock`,
`purlis-crash-*.sock`, the app's single-instance socket, or the folders holding purlis's server
sockets and pool forwards (`purlis-serve`, `engine-links`), so a socket moved while `ports` is
enforced is not reached after a `ports = "off"` you press. The same rule stops a chat **creating
any file with one of those names anywhere** (a file called `hooks.sock`, `slots.sock` or
`purlisd*.sock`): purlis's own test suite, run inside a chat, cannot make them either.
**Any folder named `purlis-serve` or `engine-links`, wherever it is, becomes unwritable to chats**
(nothing in it can be made, moved or removed), your own among them: name yours otherwise.

That call is decided **when the chat starts**, from the rules its harness will read (the
project's, and yours in the `CLAUDE_CONFIG_DIR` its profile sets). A deny rule you add mid-chat
holds a dead Bash guard closed from the next start, not before, and a settings layer reached
only through a folder the chat `cd`s into later is not counted at the start: under `open` or
`log` that window is accepted, so that `open` stays open (Q327, #7155).

**A profile or harness declaration nobody approved still asks** under every template but `open`
(Q329). Only `open`, or your own pressed `trust = "off"` or `"log"`, lets one start
unapproved, and then its chat's tab says so as it starts.

## Where the window shows it

- **A chat's pane head** says the template and how many families are set apart from it (`team
  +2`); its tooltip says the whole policy. A narrow pane drops it first.
- **Settings › Project › Policy** lists the template, each family set apart and how your chats
  run it now (stricter where a lock, a family not wired yet or a loosening that waits for your
  Accept holds it), the families an administrator locks, and Accept for a loosening. Its
  **Would have blocked** count is every chat's enforcement log, and opens it: the newest lines,
  by rule, family, chat and the `host:port` a rule was about. A line that is not as purlis
  writes it (an unknown rule, a host that is no `host:port`, another chat's) is left out and
  counted apart.
- **Where this machine's acceptance does not hold**, every family is enforced: the pane head says
  `strict`, Settings says why and offers the Accept that seals it again, and nothing says
  `watch` is the default.
- **The move to `watch`** is announced once on each machine, in the first project that names no
  template, and never while the acceptance does not hold.

The window only says the policy; the app resolves it, as below.

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
  A lock on `guards.git` also holds `guards.plane`, which took over the plane root's branch and
  reset guards (A3, A3b) from it.
- **A value purlis cannot read never loosens anything**: an unknown template is `strict`, an
  unknown mode is `enforce`, and the window says which.

## When a guard is wrong about your case

Set its family to `log` in `[policy]`, commit it, and press Accept in the window; the chat's
next start runs it that way. Or run the command yourself, in your own terminal: these guards
bound what an agent does with your authority, never what you do.
