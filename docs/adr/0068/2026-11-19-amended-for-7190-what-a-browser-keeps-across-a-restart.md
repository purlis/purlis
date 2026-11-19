# Amended for #7190, what a browser keeps across a restart, 2026-11-19

The person ruled on 2026-11-18 (#7187 items 6 to 8, #7190) what survives a restart of purlis
where a chat's browser is concerned, and what dies with it. Persisting a consent is hard to
reverse and security-relevant, so the rulings and their reasons are recorded here. The browser
quit rule (#2022, item 5 of the amendment for #2012) is narrowed by item 1; items 2 to 4 are
new. Items 1 to 4 are **ruled**; their code is `purlis_core::browserposture`,
`purlis_core::poollogins` and `app/src-tauri/src/chatbrowsers.rs`.

1. **Quitting purlis does not end a runner's browsers.** A runner is its own device, so its
   browsers outlive a quit as its chats do, and purlis takes them back by their notes when it
   reopens (the amendment for #2025). They end when the person closes them, when the runner
   stops, or when the person forgets the pool (item 5). **#2022 is unchanged for this machine:**
   a quit ends the browsers its own host runs, and the next start offers each address once. A
   runner's browser is no address the quit keeps, since it is still there. *Why:* a browser on
   a runner has a login and a page the person may be watching from elsewhere; ending it for the
   desktop's quit would be the desktop's lifecycle imposed on another device.
2. **The posture answer survives a restart, for that chat only and for the chat's life.** What
   the person pressed on a chat's first browser Ask (Allow, with the open web or not, or Keep
   off) is kept in `.purlis/app/browser-postures.json`, by the chat's identity (ADR 0066), in
   this machine's state: never the plane, never synced or committed. A resumed chat keeps the
   answer it was given, so closing a chat does not end it; a new chat has a new identity and
   asks; the answer ends with purlis's record of the chat. An answer resting on a persona's Always is not kept, since it follows that Always as
   it is now. Narrowing a chat's reach still ends its browsers and leaves the answer. **A kept
   answer is checked against the chat's current reach before it is used:** it is kept with the
   reach the chat had when it was asked, and where the reach is narrower now (a listed site gone,
   the open web or a site the person allowed gone, a site now never reached, a port now
   withdrawn) the answer is not used and is removed, and the
   person is asked again. A wider reach uses it. **Where the chat's reach is not known** (its
   sandbox's compile was not recorded) the answer is not used that time, and is kept for a call
   that knows it. A closed chat is not a chat that no longer exists: it may be resumed and keeps
   its answer. The answer goes when purlis lets go of the chat's record, which purlis checks when
   it starts against the record of open chats as it is on disk, the closed chats' records and the
   chats open, so another window's live chat is never pruned; where a record does not read,
   nothing is pruned. Both stores are written whole through a temp file renamed over
   them, under a lock file beside each, so two purlis processes never drop one another's change. There is
   no "for every chat of this persona": that is what Always is, and it is the person's own
   setting. *Why:* the answer is a consent given for one chat's task, so it is bound to that chat
   and lasts only as long as purlis keeps the chat, and it cannot be widened by persisting it.
3. **The pool-login opt-in survives a restart, on this machine only.** It is kept in
   `pool-login-optins.json` in this machine's config folder, 0600, with what it was given for:
   the persona, the login as declared (by identity, with its declared name for the Granted list),
   the pool, the pool's definition and image, the runner's device, key and image, and the engine,
   which for a pool on another machine is the pin as trusted (ADR 0123 §5). **The person's mark
   is not in what it is bound to:** it is checked at every login, as for any login on that pool
   (`Bound::over_the_link`), and an opt-in stands only where the runner is shown to be theirs at
   that moment. **Any change to the bound facts asks again**, and so does taking the mark back,
   forgetting the pool, or the person's Remove on the Granted list (audited as a revoke). A
   removal the store cannot take (another process holds it) is reported, and the opt-in no longer
   applies: it is recorded as removed in a list beside the store, which is never read past. It has no time limit,
   is never synced, committed or sent to a runner, and holds no value of any login. A Keep off
   is not kept: a refusal holds for the run alone, and replaces the opt-in it contradicts. A
   runner never holds a value or a kept login (#2021). *Why:* the opt-in was asked again at every
   restart for the same pool and persona; the person's trust is in a pool as they pinned it,
   so binding it to that pin keeps the question exactly where something moved.
4. **A runner browser's cookies live as long as the browser, and no jar is saved.** Its
   data folder is where the engine keeps them, and the host removes it when the browser ends.
   A test covers the engine ending by itself; a close and a stop end the browser the same way
   and are not tested apart. **A host that is killed cannot clear it:** the folder stays until
   the runner's own cleanup, so this is the host's doing, not a guarantee. Nothing writes a
   cookie jar elsewhere. Reusing a login across browsers is the job of kept logins (#2021),
   which never go to a runner. *Why:* a browser that outlives a quit carries its cookies with it
   and no more; a saved jar would be a login the person never opted into on that device.
5. **Browsers on a forgotten pool end, and are said so.** *Forget it here* (ADR 0123, amended
   2026-11-17) stops the pool's browsers on its machine where that answers, before the links
   end, and the event log says the person closed them, never that they crashed. A browser the
   machine did not answer for is told to the person as left running there until its runner is
   removed. The log's word is `person` (`browser.closed`), not a new one: the person's own act
   ended it, and `docs/plane-format.md`'s record of the event log is unchanged.
6. **Browser starts on different runners do not queue behind one another.** The window-wide
   start lock is gone. A start takes its runner's gate (by pool, not link, so a reconnected link keeps
   it) for the host's count against its budget alone, and holds a slot until its start answers,
   so two starts cannot both take the last place, a long engine download does not hold the next
   start, and a start on another runner waits for neither. A start that waits says so in the
   window's Notice, and Cancel refuses it as it gets its turn (#7187 item 8).
