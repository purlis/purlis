# A skill change at the person's terminal is audited by the session host

**Amends [ADR 0100](../0100-a-skill-is-installed-from-a-git-source-as-a-pinned-copy-and-never-updates-by-itself.md) as amended 2026-11-09, on 2026-11-10**, ticket #6030 (SK-11a), parent #2762. The amendment of 2026-11-09 audited every install, update and removal of a skill installed from a source on an org's project, but only the window wrote the event log (session protocol 1.8). So `purlis skill install | update | rm` of such a skill was refused at the person's terminal, and the record named a journal the host reads as the follow-up. This records the channel built instead: the terminal asks, the session host writes, and no scope gains a way to write the log.

## The channel: a typed ask of the host, over the terminal's own link

**`purlis skill` asks the session host to audit the change, as `terminal`, and makes the change only once the host says it is durable** (session protocol 1.82, `audit_skill_change`). The ask carries the change and nothing else: the act (`install`, `update` or `remove`), the level, the skills' names, the source as an audit names it, and the pin. The host resolves the project's folder, and the binary's record (`purlis_host::Record::audit_skill_change`) composes the entry, checks that the project is an org's, and writes it durably before it answers. A project without `[skill_policy]` is refused, and so is a folder that is not there. The folder need not be one the host holds: the person at their terminal may change the skills of any project of theirs, and the entry names the folder it was asked for.

- **The terminal is given no write.** It never hands the host an entry, a kind or an actor. The host writes one of the three kinds `skill.install`, `skill.update` and `skill.remove`, with the body it composes. `record`, the window's write (1.8), stays the window's alone.
- **Durable before the change, as the window's audit is.** The command waits for the host's answer. A refusal, a host that is not running, and a host older than 1.82 each change nothing, and say why.
- **The window is not needed.** The host holds the event log whether or not a window is linked to it, so the person can run the command with only `purlis serve` running. With no host at all, the command says to open purlis or start `purlis serve`.

## Why not the journal

The kill switch's journal (ADR 0071, amended by ADR 0075) is a file any process of the user can write, read by the host when it next looks, and recorded as `unattributed` for that reason. Copied here it would have cost:

- **a new store**, with a tier, a watcher and a format, for what one call already carries;
- **the audit after the change.** A line is read when the host next looks, so a change could land while its entry was not yet in the log. The log's own audit comes before every change;
- **an entry nobody can attribute.** A file is anyone's word. That is right for a stop, which only removes power, and wrong for a change to what every chat in the project is given.

An ask of the window, as a grade from a terminal is (1.63), was not needed either. A grade needs the window because the window runs it. An audit needs only the event log, which is the host's.

## Nothing a chat does writes such an entry

- **Only `terminal` may send `audit_skill_change`** (`crate::grants`). The window audits its own changes through `record`, no agent's scope (`fleet-mcp`, `approval`, `editor`) makes a person's skill change, and no link carries it: a relaying host refuses it, because the change is made on this machine and audited in its log.
- **A chat cannot be admitted as `terminal`.** Its sandbox denies it the scope's credential and the host's socket (ADR 0067 §5, ADR 0068 §5), and the host refuses every scope to a peer inside a chat (V16a). `purlis skill` refuses first inside a chat, before it asks anything.
- **A chat cannot write the log another way.** Its sandbox denies the event log's files (#873), and the kinds its hooks write are `hook.`'s alone.

## The entry's actor

**`human`, the `operator`, on the scope the change was made on**: `"actor_kind": "human", "actor": "operator", "scope": "terminal"` for a terminal's change, and `"scope": "local-ui"` for the window's, which now says so too. This is the convention the other audited acts already use (a sandbox or dispatch grant, an applied change). The terminal is a person's scope (V16a), proved by a credential no chat reaches, so its entry is the person's, unlike a `cli` line of the kill switch's journal. The residual is the one ADR 0068 §5 names for every scope: a process of the user outside every chat that reads the credential can ask as the terminal, as it can ask as the window.

## Also fixed

The log took an audited entry only of a `trust.` kind, so the window's `skill.install`, `skill.update` and `skill.remove` of 2026-11-09 were refused: on an org's project, every install, update and removal in the Skill studio failed, closed. The log now takes those three kinds beside the `trust.` ones, and no other.
