# Amendment, 2026-10-04: every item is held to purlis's app (ruling V90, #1179)

**The problem it closes.** A sandboxed project with a keyring vault started no chat. No harness's
sandbox could keep a chat off the operating system's credential store, so ADR 0067 §5's vaults
class could not be held, and both defaults together (sandbox on, keyring vaults) refused every
chat. The operator's ruling V90: hold the class at the store.

**Decision (V90a).** On macOS every keyring item purlis writes is held to purlis's app: only
the app's own binary reads it without the person's confirmation. Any other program, the
`purlis` command and every program a chat runs included, is refused or makes the Keychain ask
the person first.

- **The rule is the Keychain's own default, applied by the app's binary.** An item made with no
  access object of its own trusts the program that created it, by its code signature (measured
  above). So the app writes every item itself. The `purlis` command starts the app's binary
  beside it again, as a one-item writer that takes the item on its standard input, writes it
  and exits (`secrets::keyhold`). The writer writes and never reads, so starting it gives a
  caller nothing back. It writes only items under `charter/`.
- **An item is deleted and made again, never replaced.** A replaced item keeps the access it
  had, so one the person once let another program read ("Always Allow") would keep letting it.
  A program may delete only an item it owns, so an item the `purlis` command made before is
  deleted by the command and then made by the app. If the app cannot make it, the command writes
  it back itself, and the item is tried again at its next read. The command blocks the interrupt,
  hang-up and terminate signals from its delete to the new write and delivers them after, so
  the value is lost there only if the command is killed in a way no program can block. The app
  and the writer leave their signals alone; the writer runs in a process group of its own.
- **The command finds the app through a link.** The app's Install on PATH puts a link to the
  command in `/usr/local/bin`; the app is looked for beside the file the link names, never
  beside the link.
- **The index records it.** Each key whose item was written held carries `"held": true` in the
  keys index (`docs/plane-format.md`).
- **No new `unsafe`, and no shell-out.** A trusted-application list naming the app
  (`SecAccessCreate`) would take an FFI block, and the `security` tool's `-T` would make that tool
  the item's writer. Writing it as the app needs neither.
- **The vaults class is then held for every harness on macOS** (ADR 0067 §5 as amended): a
  Claude Code chat starts, and Codex and opencode run inside purlis's wrap, which also denies the
  credential store's service (V90b).

**Existing items (V90d).** An item whose index line does not say `held` is written again, held,
the first time purlis reads it, and its value still resolves. Only the program that made an
item can delete it, so an item the `purlis` command made is moved the next time the command
reads it; the app cannot move it. A move is made only while the key is as it was read, so it
never writes over a newer value. The vault's next `purlis secret
get`, `cp` or `exec` says once that its secrets were moved under the rule. Until its first read,
an item keeps the access it had.

**Measured on macOS 26.2, 2026-10-04,** with throwaway programs against a scratch keychain made
with `security create-keychain` and deleted afterwards (the search list was unchanged), each read
with the Keychain's dialogs off. An item made by one program and read by:

| Reader | Result |
|---|---|
| The program that made it | read |
| Another program | refused (would ask) |
| Another program, from inside a live Claude Code chat's sandbox (2.1.288, a turn against a stand-in model) | refused (would ask); the maker, in the same chat, read it |
| Any program, under purlis's own wrap (V90b) | refused: the service cannot be reached at all |

An item the `purlis` command had made was refused to the app's delete and deleted by the
command, then made by the app, after which the command was refused and the app read it.

**What it rests on, and what it does not hold.**

- **Unsigned and ad-hoc signed builds are trusted by their exact build.** A dev build, and an
  update of today's ad-hoc signed app, is a new program to the Keychain, so its first read of
  each item asks the person once, and "Always Allow" adds it. This was chosen over pinning a
  path, which would trust whatever binary is put there. Signed builds (#606) are trusted by their
  designated requirement, which survives an update; a data-protection access group follows
  them.
- **Where no app sits beside the `purlis` command** (a command-line install, or a build of the
  command alone), the command writes the item itself, and `purlis secret set` says that the
  item is not held. It stays the command's until a command with the app beside it reads it.
- **After an update of an ad-hoc signed build, neither new binary owns the items an older build
  made**, so purlis cannot move them again until signed builds. They stay held to the build
  that made them, and reads ask.
- **The `purlis` command now asks too.** It is another program, so `purlis secret get`,
  `exec` and `cp` make the Keychain ask the person before each read of a held item. Answering
  each ask keeps the rule. "Always Allow" for the command does not: it lets that build read the
  item without asking, and so lets any chat that runs the command read it silently. Resolving a
  secret in `purlisd` (V16b, #1180) takes the command out of that path.
- **The rule is about reading.** It is not an integrity boundary for the item, and the item's
  name stays visible to other programs, as every Keychain item's does. The follow-ups are
  tracked in #1180.
- **Linux (V90c).** The Secret Service keeps no per-program rule, and no harness's sandbox was
  measured keeping a chat off the session bus, so a sandboxed project with a keyring vault still
  refuses a Claude Code chat there. The refusal says how to go on: start that chat without the
  sandbox from the new-chat picker, or, for a resumed or relaunched chat too, move those secrets to a plain-file or 1Password vault, which the sandbox can keep
  from a chat. Codex and opencode wait for purlis's Linux wrap (#1040).
