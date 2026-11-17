# Amended for #6990, a browser view lists its downloads, 2026-11-17

A browser's downloads are listed to its chat by `browser_downloads`, and the window heard nothing
of them: the person could not see that a page saved a file. This note says what the view is told
of them. It goes with the amendment for #6989 of the same day, whose minor it shares. Items 1 to
3 are **decided in implementation, to be confirmed** (D-6990-1 to D-6990-3).

1. **Session protocol 1.91, the same minor as #6989.** The page frame carries the browser's
   downloads too (`PageShown::downloads`): each one's id, name, state, the bytes received and its
   size where the site said, in the order they began. The engine tells it with each frame and
   where a download begins or moves on (`Browser.downloadWillBegin`, `Browser.downloadProgress`),
   and the host sends it only where it changed. A view of a link before 1.91 is sent the frame
   without it. *D-6990-1.*
2. **Named as the chat's list names them, and nothing more.** Each name is the one
   `browser_downloads` gives (`Inner::names_kept_out`), so it never holds a value purlis keeps
   out. The view is never told a download's file, its path in the holding folder, or the origin
   it came from. What a host sends is held again as it is read (`PageShown::held`): at most 64,
   plain ids, a name cut at a character to 512 bytes, nothing received past its size. A character
   a reader would not see is left in the name and marked by the window, by purlis's one rule for
   it (#2020), never dropped unseen. *D-6990-2.*
3. **The person sees them; the chat saves them.** The header counts them ("3 downloads") once a
   page saved a file, and the count opens a list of each name with how far it came. Saving one
   into the project stays the chat's (`browser_save_download`), and the list says so: no ruling
   gives the person a way yet. *D-6990-3.*
