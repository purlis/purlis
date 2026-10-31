# Amended 2026-10-31: a sign-in's Ask ends when signed in, or when you leave the shell

**Built by #4009 and #4223**, under spec #4023 and #2030, which rule it for a pool's sign-in and for
this machine's. **Decided in implementation, confirmed by the dispatcher 2026-10-31**: it records
the rule a pool's watcher already has, so no new ruling. §3's Ask stands, except as follows.

1. **After Sign in, only two things end the watch, item 4 aside:** the harness says the profile is signed in, or
   every shell tab that sign-in was typed into has closed or its program has ended. A harness that
   cannot tell, or a runner still fetching it, keeps the Ask. Time only slows the asking: every
   few seconds for 15 minutes, then once a minute.
2. **Signed in lets every project's wait for that profile go, and each of those windows is told.**
   Leaving the shell lets go only that project's wait, and only one last refused before the shell
   was last seen open. A project closing lets nothing go.
3. **A wait with no watcher stays while its harness cannot tell,** until a signed-in answer or
   Dismiss. A chat on it still starts, as §3 has it.
4. **A profile taken off this machine or off the pool, or its pool removed, ends its watch,** so no
   look asks after it or keeps it on a runner again. Its Asks go as in leaving the shell.
5. This reverses D-4009-6, which left this machine's watcher as it was: both sign-ins now share one
   watcher.
