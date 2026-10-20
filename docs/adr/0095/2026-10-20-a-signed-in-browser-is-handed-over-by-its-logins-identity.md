# Amended for #2699, a signed-in browser is handed over by its login's whole identity, 2026-10-20

A browser that holds a login (#2022) is handed over by the login's identity, the one its kept
store is bound to (note 8 of the 2026-10-18 ruling), never by the login's name. This note adds
to that ruling; everything else stands as accepted. Each item is **decided in implementation,
to be confirmed** (D-2699-1 to D-2699-3).

1. **The identity is compared whole.** A browser signed in with a login, by purlis or opened
   from its kept store, goes to another chat only where that chat runs as the same persona and
   its own login of that name, declared and allowed now, has the same identity: the persona,
   the project's id and the declaration (its vault, keys and page). A login of the same name
   declared otherwise, in this project or another, is refused, and the browser stays with no
   chat. A browser keeps every login it was signed in with, never only the last: it goes only to
   a chat that may log in with each of them. The check reads no value from the vault.
   *D-2699-1.*
2. **A browser opened from a kept store holds that store's login from its first moment.** It is
   recorded with the store's persona, login and identity as it starts, before the login is
   tried in it, so it is never handed over as a browser that holds nothing. *D-2699-2.*
3. **The refusal names the login, never its declaration.** The person is told the chat's login
   of that name is declared otherwise, or that the chat may not log in with it now, with no
   vault, key or page in the words: why it is not allowed is not passed on. *D-2699-3.*
