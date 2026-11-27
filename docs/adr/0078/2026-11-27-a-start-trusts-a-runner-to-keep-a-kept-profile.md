# Amended 2026-11-27: a start trusts a runner to keep a kept profile until it restarts, as it shows

**Built by #7863** (map #7180, M79), on the person's ruling of 2026-11-27 on #7186 (Q1: *yes*), and
their ruling the same day on what a skipped keep must not give up (option 2, item 7).
A start on a pool readies its profile on the pool's runner over a link of its own before the
chat's own link starts it there (ADR 0095 as amended 2026-10-27, item 5). That stays. What
changes is when the profile link is made.

1. **The profile link stays separate from the chat's.** A keep goes over the desktop's own link
   to the runner, never one relayed for the window, and only to a runner shown to be the person's
   own (ADR 0095 as amended 2026-10-27, items 1 and 2). Skipping the link sends nothing new anywhere:
   the runner is sent less, never something else, and no other path carries a keep. A keep is
   skipped only where the runner shows, over the chat's own link, that it still holds the
   profile's settings as the last keep left them (item 7).
2. **A runner that kept a profile is trusted to keep it until it restarts.** This machine
   remembers, for each runner (its device id, compared without case), pool and profile, the
   digest of what the last keep sent (the profile's name, harness, pin, sandbox mark, its
   settings files and MCP servers as a runner of 1.56 or later is sent them, and the pool's
   limits) and how its harness said it stood. A start on that runner makes the profile link
   again only where:
   - the digest is not the one remembered (the profile or the pool's limits changed);
   - another device id answers for the pool (a new runner, or one made again);
   - Docker says the runner does not run (the start wakes it, `Resumed.started`), or that it
     started at another time than when it kept the profile; or
   - what is remembered was not *signed in* or *cannot tell*. A signed-out or fetching standing
     is never remembered, so a sign-in made on the runner since is seen at the next start; or
   - the keep could not leave a digest the runner can be held to: a runner before session
     protocol 1.99, or a keep that did not replace the profile's settings whole. Such a keep is
     never remembered, so every start there keeps the profile as before.
3. **What is remembered is forgotten where the runner may no longer hold it as it was**: a keep
   that failed, a profile the runner forgot (a tidy, its removal on this machine or from the
   pool), a sign-out on that pool, and a start the runner itself refused as signed out (#4233),
   which is readied again with the link.
4. **It is kept in this run of the app only**, in memory on this machine, never the plane and
   never a file, so it names no tier (ADR 0069). The first start on each runner after the app
   starts makes the link as before, so a restart of the app keeps each profile once more.
5. **Only a Docker pool's runner is remembered.** A cluster pool's runner, which its operator
   vouches for at each link, and a chat's own fresh runner (#3818), which holds nothing yet, make
   the profile link at every start.
6. **The tidy still runs as it did** (#4060): once in a run per runner, at the first start there,
   which is a start that makes the profile link anyway. It is what has a runner forget a profile
   no longer chosen for it or no longer on this machine, so it is not tied to the keep.
7. **A skipped keep is held to what the runner holds** (the person's ruling of 2026-11-27, option
   2). The keep remembered carries the digest of the profile's settings it left on the runner:
   their contents (`settings_changed.whole`, or the whole of what it sent) and their modes, each
   file `0600` and each folder `0700` as a keep writes them. The chat's start names it
   (`StartProgram.profile_held`, session protocol 1.99), carried from the very decision that
   skipped the keep, never read again from what this machine remembers, which another chat's
   sign-out or forget may empty meanwhile. Only the window or its host's relay may name it, as
   with `tools_allowed`. The runner, on the paired link the chat's start arrives on, reads the
   profile as it holds it on disk then and answers with its digest only where a keep would leave
   its settings' contents and modes as they are: the contents of its settings files, hashed the
   way the desktop hashes them, the mode of each of those files and of each folder they are in
   (one not as a keep writes it moves the digest off the one named), and nothing else a keep
   removes. That is, no file past a read's limits, no entry a read leaves out inside a listed
   folder (a link, a hidden, credential-shaped or not-text name, a skill's `node_modules`,
   `__pycache__` or `venv`), none of what a keep clears that no desktop sends (each
   harness's list: Claude Code's `hooks` and `rules`, Codex's `.env`, `AGENTS.override.md`,
   `hooks.json`, `rules` and `skills`, opencode's plugins, skills, tools, `package.json` and
   `node_modules`), no link on the way to the settings folder, and, for Claude Code, a state file
   whose user-scope MCP servers are those its kept `mcp-servers.json` names, where it keeps one,
   and in which no project has servers of its own (an empty list, which starts nothing, counts as
   none). Anything else, a settings file planted, changed, gone or with its mode changed among
   them, and a keep left
   unfinished or a profile kept for another desktop, refuses the start as `profile_changed`, and
   nothing starts. The desktop then forgets what it remembered, keeps the profile in full, and
   starts the chat once more, naming nothing. A keep that fails there is why the chat did not
   start. That profile is then never remembered on that runner again until the app restarts, so a
   difference no keep puts right (a state file no keep can read) costs one refused start, never
   one at every start. A profile whose harness makes again, as it starts, something a keep clears
   (Codex's own skills, opencode's `package.json` and `node_modules`) is never remembered at all:
   no runner that ran a chat of it could prove the profile as a keep leaves it, so every start
   there keeps it, as before.

## What stays of ADR 0095 as amended 2026-10-31

Its guarantee holds: **no chat starts over a profile's settings on a runner other than what this
machine kept there.** A chat may be able to write in a settings folder, so nothing is taken on
trust (item 1): what is remembered on this machine is only what to ask the runner, and the runner's
answer is read off its disk at each start. What is proven there is what item 7 lists: that the
settings files' contents, and their and their folders' modes, are those a keep made then would
write, and that nothing it removes is there. Anything a keep would change, a settings file
changed, planted or made runnable since the last keep, something it clears that no desktop sends,
or an MCP server it would take out, ends at a keep (item 3), which writes what it checked and clears what no
desktop sent (item 5): the skip only spares the link where the check finds nothing a keep would
do.
