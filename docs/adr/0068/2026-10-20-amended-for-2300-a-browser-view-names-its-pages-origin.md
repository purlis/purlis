# Amended for #2300, a browser view names its page's origin, 2026-10-20

Spec #2008 asks that a browser view's chip name the page and the device, and that a chat's
browser rows and thumbnails say who drives. This note says what the window is told of a page,
where that comes from, and how a row knows who drives. Items 1 to 6 are **decided in
implementation, to be confirmed** (D-2300-1 to D-2300-6).

1. **Session protocol 1.35, a minor.** A browser view's stream carries an origin frame of its own
   (`ORIGIN`, `0x03`). It holds the origin of the document the page has loaded (`scheme://host[:port]`),
   or nothing for no page or a page with no origin of its own, such as `about:blank`. The host
   sends one where the origin changes, and one before the image in every snapshot. A view of a
   link before 1.35 gets images alone. *D-2300-1, the number given by the steward.*
2. **The origin, never more.** The engine hands the host the address of the page's document from
   its own record (the frame tree DevTools reports), never anything read from the page. The host
   cuts it to its origin (`browsing::origin_of`): http and https alone, the host in ASCII, and
   never a user, path, query or fragment. A title can hold what a page shows (an account name, a
   document's name, a reset code), so the window is never sent a title, and the chip and the
   rows name none. The chip's tooltip is the origin. The window takes what a host sends as an
   origin only where it is one, exactly as `origin_of` writes it, at most 255 bytes: a view may
   be drawn from a runner's host, and the chip is purlis's own words, so any other text reads as
   no origin. A path is not shown either: it may hold a
   token. *D-2300-2.*
3. **Who drives, on a row and a thumbnail.** A chat's browser row says `you are driving` while the
   person drives it, and `purlis is signing in` while purlis performs a login in it. Its
   thumbnail carries the same mark, under it, and says so in its label. Both come from the
   window's own record: the person's take-over and hand-back, a view opened again that the host
   says the person still drives, and the login the window hands the host. So the protocol's
   listed driver does not change: it stays the agent's while purlis logs in (D-2016-2), and no
   `purlis` is added to it. *D-2300-3.*
4. **Where a page is, as its views were told.** The window keeps the origin each browser's views
   were last told. The view's chip, a row's tooltip and a thumbnail's tooltip all read it. A
   browser no view has watched says only its device. *D-2300-4.*
5. **The marks' words.** `you are driving`, as the bar says "You are driving this browser", and
   `purlis is signing in`. *D-2300-5.*
6. **One way to watch.** The browser view watches through `useBrowserWatch`, as a thumbnail
   does. That one hook draws frames, hears the origin and whether a runner's link is down, and
   hands the view what each frame shows and what its view's attachment said. *D-2300-6.*
