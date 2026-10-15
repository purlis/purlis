# Amended for #2000, a chat's hooks are heard in the session host, 2026-10-14

With the start-time switch on, the session host owns each held project's hook socket and its
chats' tokens, and what the board and Needs you are moved by comes to the window from it. Spec
#1974's grill ruled the owner: "the hook socket's owner is the session host, so the rule that a
hook admits its listener only when the listener is the chat's ancestor stays true". That is the
first of the two arrangements HP-6's item 4 allows: the host itself owns each plane's hook
socket. §6 and the amendments above are left as accepted, and this note adds to them, and
replaces #1998's item 6. Item 9 follows the operator's ruling of 2026-10-14 on the start-time
switch. Every other item is **decided in implementation, to be confirmed** (D-2000-1 to
D-2000-9).

1. **The window still decides what a line moves; the host feeds it.** The host owns the socket,
   the tokens and the admission. The board, Needs you and the asks are still the window's, and
   are moved by the lines the host hands on (item 5). Moving the live board into the host is
   #2194, with the event log, the end of a program and dispatch answers (#2003). *D-2000-1, to
   be confirmed.*
2. **The host binds each held project's hook socket with the window's own listener and checks.**
   As it first holds a project, it binds the socket at the path `hooks::socket_for` names, with
   `hookwire::Listener`. It lets the socket go once nothing holds the project. Every check on a
   line is the one the window made, in the same code: the peer's uid, the chat's token, the
   process that sent it (D-1407-6, D-1407-10), and each framed ask again (`admission_of`, #1996).
   The spool's keys are written beside the socket as before. The host is each chat's parent, so
   a hook's check of its listener (`admit_host`, HP-6's item 1) admits the host and refuses any
   other listener. The host also issues each chat its token, and composes the chat's start with
   the socket and the token itself. The window's launch says nothing of where a chat reports,
   and the token never crosses the link. The host records the program it started as the chat's.
   It forgets the program and the token as the program ends, and the token as the chat is
   closed. *D-2000-2, to be confirmed.*
3. **The host deals each chat's number.** Session protocol 1.3, a minor, adds `deal`, answered
   `dealt` with a number no chat of the project has had. It also adds `dealt_already`, which
   carries the numbers that went out before the host held the project, from the window's
   record. A number past the highest a project can count is refused, never wrapped. The window
   asks for a number before it starts a chat, and puts the chat on its board under it first, so
   the chat's first hook lands. A chat put back keeps its own number where it is free.
   *D-2000-3, to be confirmed.*
4. **One pushed word and one command carry every kind of line.** 1.3 adds the pushed notice
   `heard` (a held project's line, numbered, as JSON) and the command `reply_heard` (what the
   window answered it with). It adds no word per kind of line. A line is pushed only to the
   links that hold the project. Only `local-ui` holds a project or replies, and FD-27's table
   gains both rows. A line carries no token past the listener. *D-2000-4, to be confirmed.*
5. **Each kind keeps its shape across the link.** The window hears each line with the hearing it
   listened with itself, so every kind moves exactly what it moved before.
   - A report, a tool call and a refused commit are answered once the window has taken them,
     within a hook's own wait. A line the window does not take in time is spooled by its hook,
     as before.
   - An ask waits for its answer as long as the window takes, and the window is told when the
     asker hangs up (#1441).
   - A permission ask is held in the window until it is answered there, its hook goes away or
     its deadline passes.
   - A brokered `secret exec` runs in the window as before. Its asker's input is handed on line
     by line, and its frames are handed back.
   - A line no window is there to hear is answered as an app with nothing to answer it
     answers.

   *D-2000-5, to be confirmed.*
6. **A reply is at most one control frame.** An answer larger than the link carries is answered
   with a sentence that says so. *D-2000-6, to be confirmed.*
7. **One project's lines are heard in order, and never wait on another project's.** The lines
   heard at once go one after another on the project's own thread in the window, as the
   project's listener heard them when the window bound it (FD-9's turns). An ask, a permission
   ask and a `secret exec` each get a thread of their own. *D-2000-7, to be confirmed.*
8. **A line taken late can be recorded twice in the event log.** A hook spools a line the host
   does not answer within its wait. The window drains the spool, and a live line the host took
   late is recorded too. Which copy was recorded already (#952) is known per process. The host
   holds the live line and the window holds the drain, so each records its own copy. This holds
   until the event log is written in the host (#2003). *D-2000-8, to be confirmed.*
9. **With the switch off nothing changes**, by the operator's ruling of 2026-10-14 on the
   start-time switch: the window binds the socket and issues the tokens, as before. The
   behaviour suite's targets each have the window's hooks for a project. Three behaviours run
   on both targets, each through `purlis hook`:
   - a chat whose turn ends needs you, on Claude Code, Codex and opencode;
   - a permission ask is answered from the window, on the same three;
   - a hook refuses a listener that is not its chat's ancestor.

   *The suite: D-2000-9, to be confirmed.*
