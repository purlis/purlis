# Amended 2026-10-20: a runner chat's change to the project arrives as a proposal

**Built by #2046** on §7's row for the project (RR-17): *persona, `workspace.md` and
shared-memory changes made on the runner arrive as proposals*. §7 is left as accepted, and this
note says how. *Decided in implementation, to be confirmed* (D-2046-17 to D-2046-19).

1. **A runner chat changes the project only through the window, as a sandboxed chat here
   does.** It asks with a brokered write (ADR 0067 §2). Its line comes back over the link the
   desktop's host relays, and is heard by the window. A runner chat's sandbox denies it the
   project's own files there, as a local one's does. *(D-2046-17)*
2. **The window holds that write, and makes nothing.** A brokered write from a chat that runs
   on a pool is held as a proposal (`runnerwrites`). It is shown in the chat's tab and the Inbox
   in words: what it would do, the store or section it would write, and its text, cut at 600
   characters with how many are left (`purlis_core::runnerwrite::said`). **Apply** makes it, as
   the window makes a chat's brokered write here, for the chat as the app records it (its
   workspace and persona, never the line's). **Dismiss** makes nothing. The chat is told which on
   its next turn. There is no "always apply", and a chat has at most 16 waiting. *(D-2046-18)*
3. **Held in memory only, as proposed changes are** (PC rulings of 2026-10-14). A relaunch
   starts with none, and a chat's proposals end with it. *(D-2046-19)*
