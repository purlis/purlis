# Amended 2026-11-07: an outcome eval's chat may use a shell, held to its folder and to no host

Ticket #4476 (M65.1), the follow-up the outcome evals' amendment of 2026-10-31 named ("Outcome
chats have no shell in v1"), from spec #2762's grading sections. The person's ruling of
2026-11-07, given on the three questions the ticket left open (relayed by the chat that asked
for the work): **1a, 2a, and 3 as proposed.**

## Where purlis was

An outcome eval's chat had no shell. Its `Bash` was on its tool list only so that a call would
ask, and an ask ended the run. A skill whose job is to run a script it ships could therefore
never pass its own evals: its chat was judged on a task it was not allowed to do.

## The decision

**A skill that asks for a shell has its outcome evals graded with one** (ruling 3). A skill asks
for a shell when its `SKILL.md`'s `allowed-tools` names `Bash`, either bare or as a rule
(`Bash(sh scripts/ship.sh)`). It is read by the one frontmatter reader a grade already uses
(`purlis_skills::handed::asks_for_a_shell`), so a frontmatter purlis does not hand a chat asks
for nothing. That frontmatter's evals are left out anyway. Where the previous version a grade
compares with asks for a shell, so does the grade.

- **Every arm alike.** Each ticked eval's chat has the shell in the arm with the skill, the arm
  without it and the previous version's arm. Each arm runs the same task with the same tools, so
  the baseline is not judged with less. A judge never has a shell (§3 as amended 2026-10-31).
- **The plan says it before anything runs.** Each eval the plan offers reads "uses a shell,
  which writes only in its own folder and reaches no network", in the window and at a terminal,
  before it is ticked. The grade request carries it (`shell`, said on the wire only where it is
  so), and so does each eval chat's run-chat request (`kind` `outcome`, `shell` `true`). The
  consent's digest covers it, and so does the digest an opt-in to grading after each edit keeps,
  so a skill that starts or stops asking for a shell is seen again before it runs.
- **A skill that runs a command as it loads stays left out.** Its command runs as the skill is
  read, before anything a grade can hold or show.

**The shell reaches no host** (ruling 1a). An eval chat with a shell has its sandbox decided as
though an administrator's policy fixed no preset and no host
(`sandbox::policy::holding_to_its_folder`, inside `holding_no_kept_grants`). So none of the
project's hosts, presets, your hosts, a persona's hosts, sockets or services reaches it, and
purlis's own proxy, which carries its connections, refuses every one. The reason is in §4's
limits: a shell that reached a model's API could spend tokens the chat never reports, the same
reason a grade's chat starts no sub-agent. An eval's inputs are staged into its piece, so its
task has nothing to fetch.

**No grade's chat has a live ask** (§3's "asks nobody", made exact by the review of #4476).
Until now, a grade's chat's proxy held a connection to an unlisted host while the person was
asked (#1666). The grade's watch did not hear that ask, so the chat waited out its time. Worse,
"Allow" taken live would have widened a chat the person had consented to without it. Its proxy
now refuses such a connection instead. An Allow made elsewhere is never taken live by a grade's
chat either, as only a proxy that asks live takes one. The hold above also sets
`no_live_asks`, so the decision says the same.

**The shell writes only its own folder** (ruling 2a). That same decision gives it none of the
project's package caches: they are shared with the person's own chats, and a graded skill could
leave something there for one of them to run. Its temporary files, and Claude Code's own, go in
`.tmp/` inside its piece (`TMPDIR` and `CLAUDE_CODE_TMPDIR`). It writes nowhere else: not the
project outside its piece, not the home, not the system's temp folder. Writes by Write and Edit
stay held to its piece by their guard, as before.

**The same sandbox and consent as any chat, and no new bypass.** The shell runs under the chat's
full sandbox, in Claude Code's default permission mode with its prompts kept. Its commands run
unasked only because that sandbox holds them (`autoAllowBashIfSandboxed`), as any sandboxed
chat's do. `allowUnsandboxedCommands` stays `false`, so a command that asks to leave the
sandbox asks the person, and an ask still ends the run. Every other limit of §3 and §4 holds
unchanged: the five minutes, the token cap, the dollar limit, no sub-agent and no MCP server but
purlis's.

**Measured where it can be.** `claude_codes_own_sandbox_holds_the_settings_purlis_writes`
starts the real Claude Code that CI pins, against a stand-in model and on what purlis compiles
for such a chat, with purlis's own proxy carrying its connections. Its one command writes its
piece and its temp folder. It is refused a write in the project outside the piece, in the home,
in the system's temp folder and in Claude Code's own, and is refused a connection to a host the
project lists for its other chats. On the same project, a grade's other chats are given that host
and the project's caches, so the hold is what takes them away.

## Rejected

- **The project's hosts and caches, as any chat in the project has them** (1b and 2b). These
  are the same sandbox, but the grade's limits would not hold over what the shell spends, and a
  graded skill's writes would reach what the person's own chats run.
- **A shell for every ticked eval.** A skill that runs no command gains nothing from one, and it
  would widen every grade the person has already agreed to.
- **A tick of its own, per eval.** What a skill needs to do its job is the skill's to say, in
  the key Claude Code already reads. The plan shows it, and the consent covers it.
