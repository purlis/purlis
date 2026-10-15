# Amended for #2012, a browser is a hosted program with a view tab, 2026-10-14

A browser runs in the session host as a hosted program of the kind `browser`, and the window
watches it in a view tab. The model (a browser is a hosted program whose browser view reuses the
terminal view's snapshot and re-snapshot, frames as a JPEG screencast, only while watched, full
rate for the focused view and about one a second for thumbnails, no desktop window, Stop all
ends every browser) is the operator's ruling of 2026-10-14 (spec #2008's grill, "agree at all").
§4 and the amendments above are left as accepted, and this note adds to them. Items 1 to 7 are
**decided in implementation, to be confirmed** (D-2012-1 to D-2012-7).

1. **Session protocol 1.6, a minor.** It adds the program kind `browser` and the view kind
   `browser`; on `attach_view`, a browser view's `role` (`focused` or `thumbnail`) and `rate`
   (the most frames a second it wants), each optional; and a listed program's `kind`, given
   for a browser. A browser is listed only to a link that speaks 1.6, so a client one version
   behind never takes one for a chat. Each is recorded in the protocol's fixture. FD-27's table
   gains a row: starting a browser and attaching a browser view are `local-ui`'s alone, since a
   browser is started as the person and shows what is theirs; `remote-link` gains it with
   browsers on runners (spec #2041). *D-2012-1, to be confirmed.*
2. **A browser view's bytes are whole frames.** Each frame is its length (a big-endian `u32`
   counting what follows), a format byte (`1`, JPEG) and the image. The snapshot is the latest
   frame so framed, or nothing before the browser has made one, and the live bytes are the
   frames after it, so a view attached again after it fell behind starts on a whole frame. A
   reader passes over a frame of a format it does not know, so a later minor may add one.
   *D-2012-2, to be confirmed.*
3. **Frames only while a view is attached, as often as the most eager view wants.** The first
   view asks the engine for frames and the last to go stops them. The engine's next frame is
   asked for no sooner than the pace of the view that wants frames most often: every frame for
   a focused view, about one a second when only thumbnails are attached. Between two frames a
   view is sent only the newest, never a backlog. The engine is asked to show, pace and hide
   under one lock, so a view attached as the last one detaches is still sent frames. The
   window's view tab closes its view while nobody can see it (its project is not in front, or
   the window is hidden) and attaches again when it can be seen. *D-2012-3, to be confirmed.*
4. **What starts a browser is the binary's.** `purlis-host` defines the seam (`Browsers`, and
   `Screen` for a running browser); the window's binary and `purlis serve` hand it the core's
   (`purlis_core::browsers`), which starts the pinned engine (installed in
   `<data>/engines/chrome-for-testing` on first use) with the binary itself as its keeper and a
   reach proxy as its only route, so the host crate still links neither the engine nor the
   core. **The window's host now runs core code**: the engine's download and the data home are
   read in that process, as a browser starts and never before, so a host that starts no browser
   reads neither (amends item 1 of the amendment for #1997). Each browser's data folder is its
   own under `<data>/browsers`, and the host removes it once the browser has ended; a chat's
   sandbox denies reading and writing that folder (ADR 0067 §5, class 3), and a chat started
   without the sandbox is not held to it, as with the other paths of that class.
   *D-2012-4, to be confirmed.*
5. **The person opens a browser, in their own window.** The window's New browser starts one in
   the project in front and opens its view tab; the commands are the window's alone, and the
   protocol refuses a browser to every other scope (item 1). The palette does not offer New
   browser until a browser the person opens can go somewhere (#2176). Until browsers have rows
   and pins (#2024, #2022; #2205), a browser lives as long as a tab shows it: when the last such
   tab closes it ends, and letting go of its project ends every browser the window started in
   it. Chats get browser tools later (#2013). *D-2012-5, to be confirmed.*
6. **Stop all ends every browser.** A browser is a hosted program, so the host's kill switch
   (#2001) ends it with every other program, which ends the engine's whole process group, and
   refuses a browser's start while it is thrown. The window also stops each browser its host
   lists when it throws the switch or hears `purlis stop --all`. *D-2012-6, to be confirmed.*
7. **One table of hosted programs.** A browser is a row of the same table as the host's chats
   and shells (#1998's `Hosting`): listed, stopped, closed and ended with the host by the same
   code. A hosted program gives a view of a kind, and the size its views are drawn at, where it
   is not a terminal. *D-2012-7, to be confirmed.*
