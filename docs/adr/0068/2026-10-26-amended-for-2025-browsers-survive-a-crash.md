# Amended for #2025, browsers survive a crash of the window, 2026-10-26

#2006's amendment left browsers out: "A browser a project holds is not reattached yet". Spec
#2008 asks that a crash cost a browser nothing (its story 49). This note brings browsers into
§2's crash row. Items 1 to 6 are **decided in implementation, to be confirmed** (D-2025-1 to
D-2025-6).

1. **The window keeps a note of each program in its host (session protocol 1.42, a minor).**
   What only the window knew of a browser (whose chat it is, what the chat calls it, whether it
   is pinned, the logins it holds, that the person took it over) is in the window's memory, and
   a crash loses it. The host keeps a copy: a program's `note`, given on its start or replaced
   by `note_program`, the window's own words, which the host keeps beside the program, never
   reads, and answers back with it to `programs_of`. A note is at most 16 KiB; one past that is
   refused `too_large`. Both are `local-ui`'s alone, the `programs_of` row of FD-27's table: the
   note decides what a window takes back, and only the window takes programs back. The host
   keeps notes in memory only: a host that ends ends its browsers too. *D-2025-1.*
2. **`programs_of` answers a project's browsers too**, to a link of 1.42 or later, each with who
   drives it (the lease the host holds) and its note. A link before 1.42 is answered harnesses
   and shells alone, as before. *D-2025-2.*
3. **A window that comes back takes each browser back once the project's put back has
   settled**, after #2006's item 8 closed the chats nothing reattached: a chat's browser goes
   back to its chat where that chat is open in this window again, under the name it knew,
   with its chat's answer to its browser Ask, so the chat's next call acts on it and asks
   nothing; a pinned one whose chat did not come back is listed under Pinned browsers, as one
   whose chat closed is; the person's own comes back in a view tab of its own. *D-2025-3.*
4. **What nothing takes back is ended** with the project's put back: a browser with no note (a
   window that crashed before it noted it), one whose note does not read, one whose engine
   ended while no window was there, and a chat's whose chat did not come back. Nothing runs with
   no row or tab. *D-2025-4.*
5. **The lease is as it was.** The host held it through the crash (#2014): a browser the person
   drove is still theirs, marked so on its row from the first listing and never stopped for
   going unused, and a view opened again says so. The engine keeps its epoch, since it is the
   same engine; no path starts an engine again for a browser that ran before. *D-2025-5.*
6. **A view attached again draws from the next whole frame**, as every browser view does
   (#2012): nothing new. What the logins held and the take-over record come back with the
   browser, so a hand-over still checks every login it was signed in with and no quit keeps
   where a browser the person drove was. *D-2025-6.*

**What this does not do.** A chat's browser opened in a view tab of its own before the crash
comes back on its chat's row and tab, not in that view tab: the window keeps no record of view
tabs across a launch. A chat's browser sites, allowed for that chat alone while the window held
it (#2018), are memory only, as before, and are asked again.
