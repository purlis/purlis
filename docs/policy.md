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

| Template | What runs | Sandbox |
|---|---|---|
| `open` | nothing | none |
| `watch` (the default) | everything logs; `secrets`, `vaults` and `guards.credential` are enforced | the floor |
| `hold` | `watch`, and `guards.merge` enforced | the floor |
| `team` | `guards.merge`, `guards.release`, `guards.save`, `guards.credential`, `secrets`, `vaults` and the families Q308b added enforced; the rest log | the full sandbox |
| `strict` | everything enforced: purlis before Q308 | as `[sandbox]` says |

The families, and what each covers, are listed in
[`docs/plane-format.md`](plane-format.md#policy--which-of-purliss-refusals-run). In this release
the git, merge, release, save and credential guards, `secrets`, `processes`, `dispatch` (a pair
nothing covers starts with no grant) and `asks` (a host is answered Allow) switch; every other
family is a key already and stays enforced until it is wired. Your own denies hold in every mode,
`open` included: a dispatch never and a host you kept blocked.

## Who can change it

- **A tightening applies at once**, to every chat, from its next tool call.
- **A loosening waits for your press.** The window names each family the project now runs looser
  than you accepted, with **Accept**. Until you press it, your chats keep what you accepted
  before. What you accepted is kept on this machine, sealed by purlis's app; a chat can neither
  press Accept nor write it.
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
