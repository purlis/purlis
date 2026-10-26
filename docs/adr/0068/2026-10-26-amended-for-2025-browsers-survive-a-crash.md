# Amended for #2025, browsers survive a crash of the window, 2026-10-26

#2006's amendment left browsers out: "A browser a project holds is not reattached yet". Spec
#2008 asks that a crash cost a browser nothing (its story 49). This amendment brings browsers into
§2's crash row. Each item is **decided in implementation, to be confirmed**, and item *n* is
D-2025-*n*.

1. **The window keeps a window note of each browser in its host (session protocol 1.39, a
   minor).** What only the window knew of a browser (whose it is, its chat's identity and what
   the chat calls it, the chat's Ask answer and the persona whose Always it rests on, whether it
   is pinned, the logins it holds, that the person took it over, that a view tab of its own
   showed it) is in the window's memory, and a crash loses it. The host keeps a copy beside the
   browser: a `ProgramNote`, given on its start or replaced by `note_program`, the window's own
   words, which the host never reads and answers back to `programs_of`. A note is at most
   16 KiB; one past that is refused `too_large`, and the one before is kept. Both are
   `local-ui`'s alone, the `programs_of` row of FD-27's table, and `note_program` is refused for
   a program in a project the link does not hold. The host keeps notes in memory only: a host
   that ends ends its browsers too. Not the host record (#2007), which the host writes for
   itself.
2. **`programs_of` answers a project's browsers too**, to a link of 1.39 or later, each with
   who drives it (the lease the host holds), its window note, and `logged_in`: the host's own
   record that a login was performed in it or it opened from a kept login, which no note can
   unsay. A link before 1.39 is answered harnesses and shells alone, as before.
3. **A window that comes back takes each browser back once the project's put back has
   settled**, after #2006's item 8 closed the chats nothing reattached, from the browsers the
   host answered at the window's first hold alone, so a re-link never hands back one the window
   started since. A browser the window already holds is left as it is, its logins merged.
4. **What nothing takes back is ended** with the project's put back: a browser with no note,
   one whose note does not read, one being ended when the window went, one whose engine ended
   while no window was there, a chat's whose chat did not come back and that was not pinned,
   and one the host says was logged in whose note names no login. Nothing runs with no row or
   tab, and no browser's login record is lost.
5. **Notes are sent from one place**: after each command of the window's, the note of every
   browser whose note changed, in order, on the project's own link, never waited on. A browser
   let go of to be ended is noted so at once. The host's `logged_in` record, not the order of
   notes and logins, is what keeps a login from being forgotten across a crash.
6. **A chat's browser goes back only to the same chat, and only with what that chat may hold
   now.** The note names the chat by its number and its identity (ADR 0066); a chat open under
   that number with another identity does not take it. One holding logins goes back only where
   the chat may log in with each now, as a hand-over checks (#2699); else it is the person's,
   under Pinned browsers, its logins kept. A pinned one whose chat did not come back is the
   person's too.
7. **An Ask answer that rested on Always follows Always as it is now.** The note carries the
   persona the answer rests on; the browser goes back resting on it, and is held at once to
   that persona's Always as this machine keeps it now, so a narrowing made while no window was
   there ends it (#2241, D-2013-11).
8. **The person's own browser, and any a view tab of its own showed, comes back in a view
   tab.** The window says which browsers a view tab shows as its tabs change; once the window
   has laid its tabs out from its record, it asks for the browsers to give a tab (a pull, read
   again as browsers come or go), so a window that draws late still gets each. A chat's comes
   back on its row and its chat's tab as well.
9. **The lease and the picture are as they were.** The host held the lease through the crash
   (#2014): a browser the person drove is still theirs, marked so from the first listing and
   never stopped for going unused, and the engine keeps its epoch, since it is the same engine.
   A view attached again draws from the snapshot the host keeps, the latest whole frame, so a
   page that has not moved since draws at once.
10. **The window end-to-end test runs in a run of its own**, on macOS and Linux, because its
   host downloads the engine on its first browser: a browser view tab draws the engine's page
   on its canvas, and a press on the page takes the browser over.
11. **A chat's browser sites, allowed for that chat alone (#2018), stay memory only**, as
   before, and are asked again after a crash; #3403 keeps them.

**What this does not do.** A runner's browsers after a crash of the desktop (#3380). No test
yet kills the real app with a real engine and starts it again (#3404).
