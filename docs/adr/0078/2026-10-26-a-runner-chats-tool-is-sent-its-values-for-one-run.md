# Amended 2026-10-26: a runner chat's tool is sent its values for one run

**Built by #2047**, under spec #2041 (stories 14 and 21; *"Secrets on runners: every vault value
is sent per command from the desktop through its approval gate (ADR 0078 §7); with the desktop
away the command waits as an Ask. No vault value is stored in a cluster."*). Items 1 and 2 apply
the rulings of 2026-10-14 (a runner not marked owned never receives a vault value; an owned
device receives one only with a grant) and the dispatcher's ruling of 2026-10-26 on D-2050-4.
Item 3's per-run Ask is **the person's ruling of 2026-10-26 on story 21**: every run of a tool on
a runner asks the person, with no standing Allow; #3409 may relax it later, on their word. The
rest was decided in #2047 and is **decided in implementation, to be confirmed** (D-2047-1 to
D-2047-8 in the pull request). §7's table row for secrets stands, read as below.

1. **What a runner chat may be sent a value for: its persona's declared tool, through `purlis
   secret exec`, and nothing else.** A credential is never granted to a chat. The runner's host
   finds which of the tools its own start armed the command is, and refuses what is none as a run
   on this machine is refused, before the desktop is asked anything.
2. **Only a runner shown to be the person's own is sent a value**, by the one check browser
   logins use (amendment of ADR 0089, 2026-10-26, item 3): a Docker pool whose engine is a local
   socket on this machine, until a pool can be marked owned (#2561). A hosted runner is never
   sent one.
3. **The desktop's gate is the window's, at each run, and each run asks the person.** The tool
   must be one the persona declares and this machine allowed, asked for with exactly its vault
   and hand-over; the vault one the persona may use here; the chat started in its sandbox, as a
   persona. Then the person answers one Ask for that run (Allow this run, or Keep off), and **the
   command waits for the answer**, at most 15 minutes. On Allow the pool and its runner are read
   again and must be as the Ask showed them.
4. **The values cross once, for the run, bound to it.** The runner's host numbers each run with
   128 random bits; the window reads exactly the tool's keys and replies them to the runner's host
   on the line it handed on, with the chat, the tool and that number, and the host runs nothing on
   values that are not for that run. The host numbers its lines from a random start, so a reply
   meant for a host before it finds nothing. The runner's host runs the tool itself, held to its
   own record of the chat's sandbox, as the window runs one for a chat of this machine, with no
   core file. A file credential is a 0600 file where chats are denied, removed as the run ends;
   nothing else of the values is written on the runner. A runner of an earlier purlis never names
   the tool or the run, and is sent nothing.
5. **With the desktop away the command waits on the runner**, at most 15 minutes, told so on its
   stderr, admitted within one bound it shares with the browser logins that wait (16, 4 of one
   chat's). Once a window hears the runner again it is handed on marked as one that waited, and
   its Ask says it was asked while the person was away. It is let go of, and runs nothing, once
   its asker hangs up or its chat is gone; an asker that hangs up while the person is asked is
   told to the window, which lets the Ask go and reads no vault for it. The chat may carry on
   meanwhile: only the command waits.
6. **The runner's host keeps, in memory, what it composed for each chat in a sandbox** (its
   persona, folder, harness and compiled sandbox with the tools armed), until the chat's hook
   channel forgets it or a chat is composed under its number without a sandbox. It holds no
   value, and it is no store on disk.
7. **The desktop's record of a value it sent, `secret.forwarded` on its chain (§10), follows in
   #3406.** Until then the runner records each run as any brokered run is recorded.
