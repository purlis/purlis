# Amended for #2613, a login runs its page scripts in purlis's own world, 2026-10-20

The amendment for #2016 says how purlis performs a login. This note says where its scripts run;
nothing else changes, and the session protocol does not.

1. **Every script a login runs on its page runs in purlis's own world.** The fields are found by
   scripts run in purlis's world in the page's own document, and focused, asked whether they
   have the focus and measured through that world's objects. A field is cleared by a function
   run in the world by its unique id, given the field resolved into that world, so the engine
   refuses an element of any other world and the world is made again once. So every script a
   login runs is in purlis's world, as every other script purlis runs on a page has been since
   #2138. What the page's scripts redefine does not decide
   which field is filled. *D-2613-1, decided in implementation, to be confirmed.*
2. **No world, no fill.** A world the engine no longer has is made again once; where none can be
   had, the page is taken as unreadable: no value goes in, the login fails in words once its
   time is up, and the browser is handed back to the agent. Where the world ends after a value
   went in, the fill's own checks fail and the browser is ended, as for a page that changed.
   A page that cannot be read is never taken for one signed in from kept logins. *D-2613-2,
   decided in implementation, to be confirmed.*
