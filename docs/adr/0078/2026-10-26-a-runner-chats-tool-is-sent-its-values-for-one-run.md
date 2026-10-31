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
   on this machine is refused, before the desktop is asked anything. **Nobody on a runner can
   answer the persona's Allow**, so its desktop's start says what its person allowed there, as a
   digest (session protocol 1.55, #4071). The runner arms the persona's tools only while that is
   the digest of the persona as it reads it, and the start came over a link. The digest holds
   for that one start and is kept nowhere, so an Allow taken back holds from the next start. It
   arms tools alone, never the persona's hosts.
   A tool armed so is sent nothing but through item 3's gate. *Decided in implementation, to be
   confirmed* (D-4071-6).
2. **Only a runner shown to be the person's own is sent a value**, by the one check browser
   logins use (amendment of ADR 0089, 2026-10-26, item 3): a Docker pool whose engine is a local
   socket on this machine, or, by the person's ruling of 2026-10-30 (built by #4071), a cluster
   pool the person marked as their own whose claim the operator vouches for now, at the gate and
   again after Allow, failing closed where the cluster does not answer. A GitHub App's minted
   token goes by the same check. A hosted runner is never sent one. Allow is bound for a cluster
   runner to its pool's name, context, cluster uid, operator key, claim uid, pod, device and link
   key, and the Ask's engine names the cluster and context; there is no image digest, and any
   change refuses the run, as item 3 says.
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
7. **The desktop's record of a value it sent, `secret.forwarded` on its chain (§10), is built by
   #3406.** Once the values, measured to fit the link's frame, are handed to the link, the window
   writes `secret.forwarded` under the chat, naming the vault, the keys, the device id of the
   runner the chat's link reached (item 9), its pool, the chat, its persona, the tool and the
   request it answers, never a value. A reply the session host refuses after that is told only in
   its log, so the record may over-record. No audit chain is written yet (AU-3, AU-5), so it is
   recorded where every secret handed out is recorded until AU-5 audits them, the project's
   trace, and kept by the 30-day sweep as they are (V71); like them, it is not tamper-evident
   until AU-5. The runner records the run
   as any brokered run is, saying its values came from its desktop, with the same request. The
   request is named by a digest of the run's number, never the number itself. *Decided in
   implementation, to be confirmed* (D-3406-1 to D-3406-3).
8. **Every copy of a value sent for a run is wiped from memory once it is no longer needed**
   (#3420), on the runner and on the desktop, its own brokered runs too: as the window reads it,
   as the run reads it, every copy a run is prepared with, and the window's measure of its reply,
   on success and on every refusal. The reply is then the session protocol's to carry; its
   frames follow in #3764. *Decided in implementation, to be confirmed* (D-3420-1).
9. **A runner chat is judged by the runner its link reached** (#3417, session protocol 1.48). The
   desktop's host answers a window's `relay_to` with the runner it resolved the name to, as
   `<pool>/<runner>`, and the window keeps that name with the link, apart from the pool's name it
   routes chats by. A chat's tool values, the pool read again after Allow, its browser logins'
   opt-ins and `secret.forwarded`'s device id are bound to that runner as this machine keeps it:
   for a chat that stayed on a draining runner after its pool moved on (amendment of
   2026-10-21), the draining runner, never the pool's runner now. A host before 1.48 names no
   runner: then a chat is judged by the pool's runner only while none of the pool's runners
   drains, and with one draining it is sent no value. A move and a new chat go by the pool's own
   name, which reaches no draining runner. *Decided in implementation, to be confirmed*
   (D-3417-1, D-3417-2).
10. **A runner chat's tool run is held to the chat's sandbox as it is now** (#3407). The
    runner's record of the chat (item 6) keeps a reader of the chat's own confinement on the
    runner, found in that host's hold by its id and the program that started it. Each run is
    held to the sandbox as it was compiled at the start, plus the hosts that confinement's board
    allows live as its values come, read again then, so a change while the person was asked
    holds for that run, and a chat gone meanwhile runs nothing. So an Allow on a block's Notice
    for the chat reaches its tool's next run, and a Forget narrows that run again, as
    `confines_now` holds a run on the desktop. Only the
    window's answers change the board, so nothing else can widen a run. A confinement that has
    ended, or one held under its id for another program, allows nothing live. *Decided in
    implementation, to be confirmed* (D-3407-1, D-3407-2).
11. **A runner chat's tool Ask goes once its command has gone** (#3408). An asker that hangs
    up is told to the window (item 3). Beyond that, when the link to a runner goes down, the
    runner lets go of every line it handed on, and the window ends each of that runner's
    commands that wait on it (`link_down`). Their Asks go, no vault is read for them, and
    nothing is sent. Values allowed for a command that went after the answer are never handed
    on once the window has heard it went, and nothing is recorded as sent. A command that goes
    in the moments before the window hears so may still be handed its values, which its runner
    runs nothing on and keeps none of. That is the over-record class of item 7 (D-3406-2).
    *Decided in implementation, to be confirmed* (D-3408-1, D-3408-2).
