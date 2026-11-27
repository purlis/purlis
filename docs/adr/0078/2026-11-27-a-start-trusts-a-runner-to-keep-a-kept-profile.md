# Amended 2026-11-27: a start trusts a runner to keep a kept profile until it restarts

**Built by #7863** (map #7180, M79), on the person's ruling of 2026-11-27 on #7186 (Q1: *yes*).
A start on a pool readies its profile on the pool's runner over a link of its own before the
chat's own link starts it there (ADR 0095 as amended 2026-10-27, item 5). That stays. What
changes is when the profile link is made.

1. **The profile link stays separate from the chat's.** A keep goes over the desktop's own link
   to the runner, never one relayed for the window, and only to a runner shown to be the person's
   own (ADR 0095 as amended 2026-10-27, items 1 and 2). Skipping the link sends nothing new anywhere:
   the runner is sent less, never something else, and no other path carries a keep. **What a
   skipped keep gives up is open, for the person to rule** (see the last section): a skipped keep
   does not rewrite the profile's settings on the runner.
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
     is never remembered, so a sign-in made on the runner since is seen at the next start.
3. **What is remembered is forgotten where the runner may no longer hold it as it was**: a keep
   that failed, a profile the runner forgot (a tidy, its removal on this machine or from the
   pool), a sign-out on that pool, and a start the runner itself refused as signed out (#4233),
   which is readied again with the link.
4. **It is kept in this run of the app only**, in memory on this machine, never the plane and
   never a file, so it names no tier (ADR 0069). The first start on each runner after the app
   starts makes the link as before.
5. **Only a Docker pool's runner is remembered.** A cluster pool's runner, which its operator
   vouches for at each link, and a chat's own fresh runner (#3818), which holds nothing yet, make
   the profile link at every start.
6. **The tidy still runs as it did** (#4060): once in a run per runner, at the first start there,
   which is a start that makes the profile link anyway. It is what has a runner forget a profile
   no longer chosen for it or no longer on this machine, so it is not tied to the keep.

## Open: what ADR 0095's rewrite at every keep defended

ADR 0095 as amended 2026-10-31 holds that *"nothing is remembered, since a chat may be able to
write in a settings folder"* (item 1), that *"a file changed or planted since … ends there"*
(item 3), and that every keep writes what it checked, even where nothing differs (item 5). Each
start's keep is therefore what puts a profile's settings on a runner back as the desktop has them.

A start whose keep is skipped puts nothing back. A settings file that a chat on the runner changed
or planted in the profile's settings folder, a permission allowed among them, stays for every later
chat on that profile there until the runner restarts or the profile changes on this machine. The
ruling on #7186 Q1 weighed the time saved, not this. Until the person rules on it, this amendment
is not in force: either a skipped keep is accepted with this trade-off written here, or a start
skips the link only where the runner can show, over the chat's own link, that it holds the digest
it was last sent.
