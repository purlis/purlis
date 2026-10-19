# Ruled 2026-10-18 (the person's rulings on #2021): a profile keeps its browser logins sealed on this device

§1 says a profile holds "the browser logins kept under it". The person ruled on 2026-10-18 how,
and #2021 builds it. The text above is left as accepted, and this note adds to it.

1. **Where.** `<data>/browser-logins/<profile>/<login>/state.sealed`: device-bound, never
   committed, sent or backed up; every chat is denied it, reading and writing; wiped when its
   profile is removed or forgotten. Never in a profile's own folder, which for an adopted profile
   is the harness's (§2). *Ruled by the person, 2026-10-18.*
2. **Its key.** One keyring item per device, `purlis/@browser-logins/<device>`, made on first use
   and held by the binary alone. *Ruled by the person, 2026-10-18.*
3. **When it is used: on `browser_login` alone.** A chat's browser that performed a declared login
   becomes that login's kept store when it ends, whether closed, stopped for going unused, or at
   quit. The next chat of the same profile whose `browser_login` of that login is allowed gets a
   browser opened from it, already signed in, held to the login's site from its first moment;
   purlis fills the login only where the page asks for it. *Ruled by the person, 2026-10-18.*
4. **How it is sealed.** An archive of the browser's own data, less what it caches, its own home
   and its log, sealed with ChaCha20-Poly1305 (`ring`) under the device's key. Its additional
   data is the store's own place and the generation it was written at; the keyring item holds
   the key and each store's last generation, so a store moved elsewhere, or an older copy put
   back, does not open. A store that does not open is never half opened: the browser starts
   fresh. *Decided in implementation, D-2021-1, to be confirmed.*
5. **One writer.** A lock beside each store: the browser that holds it writes the store back
   once its engine has ended; any other browser opened from it meanwhile is a copy, its changes
   discarded, and the chat is told so. *Decided in implementation, D-2021-2, to be confirmed.*
6. **What an earlier browser left behind** (as #2204 sweeps): a browser folder that holds a kept
   login has a mark beside it, `<data>/browsers/<program>.kept`, made and locked before anything
   of the store is written into the folder and held while the browser runs. Before a store is
   opened, and when a profile is removed, every folder whose mark names it and that no running
   browser holds is removed, with what it was being opened into. A browser whose profile was
   removed while it ran writes nothing back. *Decided in implementation, D-2021-3, to be
   confirmed.*
7. **A chat's profile** is the one its record names, else its harness's built-in, whose name an
   adopted profile keeps. *Decided in implementation, D-2021-4, to be confirmed.*
8. **Whose login a store is.** A store is one login as declared and allowed, never only its
   name: `<data>/browser-logins/<profile>/<persona>/<login>-<identity>/state.sealed`, where
   `<identity>` is a digest of the persona, the project's id and the login's declaration (its
   vault, keys and page), each field tagged and length-prefixed under a versioned domain, so how
   a login is worded to the person is no part of it. A login of the same name of another persona or project, or declared
   otherwise since, has a store of its own. This is ruling 1's place, made precise. *Decided in
   implementation, D-2021-7, to be confirmed.*
9. **Left out of backups.** `<data>/browser-logins/` and `<data>/browsers/` carry the exclusion
   macOS's backup reads (`com.apple.metadata:com_apple_backup_excludeItem`, as `tmutil
   addexclusion` sets it), set as each folder is made. Every folder of a store's place is this
   user's alone, `0700`, and never a link. *Decided in implementation, D-2021-8, to be confirmed.*
10. **A browser keeps one login.** A browser keeps a login only where it started fresh or from
   that login's own store. *Decided in implementation, D-2021-9, to be confirmed.*
11. **One writer of the key.** The keyring item is read, changed and written under one lock,
   `<data>/browser-logins/.keys.lock`, held by every write of a store and by a profile's wipe.
   *Decided in implementation, D-2021-10, to be confirmed.*
12. **A running browser's folder is plain.** The engine runs with a mock keychain and a basic
   password store, so while a browser runs, and after a crash until the next sweep, its folder
   under `<data>/browsers/` holds its cookies and storage in a form any process of the same user
   can read with no keychain prompt: weaker than a stock Chrome profile, whose cookies the
   keychain guards. It is held by every chat's sandbox denying the folder, by `0700`, by its
   exclusion from backups, by sealing what is kept when the browser ends, and by the sweep of
   what a browser that ended outright left behind. *Decided in implementation, D-2021-11, to be
   confirmed.*
