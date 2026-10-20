# Amended for #2693, kept browser logins tightened further, 2026-10-20

Six items raised in the reviews of #2021, each built or decided against. This note adds to the
2026-10-18 ruling; everything else stands as accepted. Each item is **decided in
implementation, to be confirmed** (D-2693-1 to D-2693-6).

1. **A store is opened under the keyring's lock.** The sealed file and its generation are read
   as one under `.keys.lock`, which a write-back holds from its read of the generation to its
   write, so a browser opened while another writes back reads both as they were or both as they
   are, and never starts fresh by mistake. The unsealing itself is outside the lock. *D-2693-1.*
2. **A removed profile is never made again by a browser that outlived it.** Removing a profile,
   whether it kept a login or not, writes `.removed-<profile>` beside the stores, holding the
   time of the removal from the same clock a browser's beginning is read by, replaced whole. The
   removal writes it under the keyring's lock before it decides anything else. A store's hold is
   taken under that lock, and is refused, with nothing made, for a browser that began before
   that time, and a write-back checks it again under the lock, so a browser that took its hold
   just before the removal writes nothing either. A browser that begins after it, as a new
   profile of the name's, keeps as any. *D-2693-2.*
3. **No kept logins without the list.** A build whose list of what of a browser's folder is not
   kept is empty (any build off unix today) refuses to seal, open or hold a store. *D-2693-3.*
4. **Linux backups are documented, not excluded.** Linux has no standard exclusion that every
   backup reads, so purlis sets none there, and `docs/plane-format.md` says which folders a
   person who backs up `$XDG_DATA_HOME` leaves out. *D-2693-4.*
5. **Only a fresh browser is kept.** A browser that acted, or that the person took over or typed
   into, before its first login is not kept as that login's store, and the chat is told to log
   in from a new browser to keep it. A browser opened from a login's own store keeps it however
   it browsed since. Any act counts, a read included, so nothing a page did in between is kept;
   a take-over or an input the browser refused does not. A browser keeps a login only where
   every login tried in it was that one, so a session of another login, tried before and not
   kept, is never sealed in under it; trying the same login again keeps it. *D-2693-5.*
6. **Other sites' cookies are not filtered out: decided against.** Chrome keeps every site's
   cookies in one SQLite database (`Default/Network/Cookies` in current builds), and each
   origin's storage in shared LevelDB and IndexedDB folders, so keeping one site's would mean
   rewriting those files in the formats of the Chrome build that wrote them. That needs new dependencies and breaks silently
   when Chrome changes them, or needs a clear over the DevTools protocol before the engine ends,
   which a crash or an idle stop skips. With item 5, what is kept is what the login itself
   produced: the login's own site, and the sites it signed in through (an identity
   provider's), which the login needs to stay signed in. A kept browser is
   held to that one site for good, so nothing else it holds is reachable. *D-2693-6.*
