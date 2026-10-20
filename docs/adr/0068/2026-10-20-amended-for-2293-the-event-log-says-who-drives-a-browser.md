# Amended for #2293, the event log says who drives a browser, 2026-10-20

Spec #2008's Events ask that the host's event log say when a browser's driver changes and that
the person or purlis typed, never what, and never carry page content or values. This note says
which events are written, by whom, and what is in them. The session protocol does not change.
Items 1 to 6 are **decided in implementation, to be confirmed** (D-2293-1 to D-2293-6).

1. **Three kinds.** `browser.driver_changed` says who drives a browser now: `agent`, `person`
   (who took it over) or `purlis` (while it performs a declared login, #2016).
   `browser.typed` says that `person` or `purlis` typed into it. `browser.handed_over` says the
   person handed a browser no chat used to a chat. Each body is the browser's program id and that
   one word, nothing else. *D-2293-1.*
2. **The window says them, the host writes them.** Who a browser belongs to is the window's to
   know: a chat's browsers, a pinned one past its chat, a hand-over. So the window tells the log's
   one writer, as it tells it every write it decides (`Entry::Browser`), and the event is under
   the chat whose browser it is, the receiving chat for a hand-over, or no chat for a browser no
   chat uses. Every change of driver goes through the window's own commands: a take-over, Done,
   an address the person sends (which hands the browser back first), and a login. *D-2293-2.*
3. **Purlis is a driver in the log.** While purlis logs in, the host's lease stays the agent's
   epoch and the protocol lists the browser as the agent's (D-2016-2), but the log says `purlis`
   from the moment the login is sent until it answers, and `agent` after, unless the person took
   the browser over meanwhile. A login the host refuses because the person drives writes
   nothing. *D-2293-3.*
4. **Typed once a stretch, never what.** The person's first key pressed after each change of
   driver writes one `browser.typed`; a click is not typing. A login that went through writes one
   for purlis. There is no key, count, field, value, address, title or page text in it: a count
   of keys would tell a password's length. *D-2293-4.*
5. **A closed set the writer composes.** The window sends a variant and a browser's id. The
   writer refuses a browser named by anything but a program's id (letters, digits, `-`, `_`, at
   most 64), reads no field more, and composes the body itself. *D-2293-5.*
6. **Best effort.** A take-over, Done or a login is never refused because its event was not
   written: the write is warned of in the window's log. A window started again reads who drives
   from the host and does not say it again. *D-2293-6.*
