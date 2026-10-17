# Amended 2026-10-16: a fourth send, when the person hands a browser back (#2015)

Spec #2008, as ruled on 2026-10-14: "the browser needs you" is an Ask whose source is a chat's
browser, and Done wakes the chat with "the person finished". This is a fourth send beside the
three above, and it is held to the same gate as the second.

A chat asks the person to take one of its browsers over through `ask_operator` pointed at that
browser. The tool returns at once; the Ask waits under Needs you, and while it waits the chat's
acts on that browser are refused in words. When the person hands the browser back (Done, or the
browser view's one chord), the Ask ends and the chat is told one line:

> ⬢ purlis: your ask that the person take over browser `b1` — They finished and handed it back —
> take a fresh snapshot before you act: references from before are void.

- **It is purlis's own fixed text**, from `purlis_core::askoutcome`, with one name in it, the
  browser's, which the app gave it. Nothing of what the person did in the browser is in it: not
  what they typed, not where they went, not what the page showed.
- **It follows the person's own press on one control**, the hand-back, and only where an Ask of
  the chat's waited on that browser. A take-over the person started themselves tells the chat
  nothing (D-2014-10).
- **When it is sent is the board's answer, never the screen's**, exactly as for a vault's Allow:
  a chat that is waiting and asking nothing is sent it now, one bracketed paste and then Enter;
  a chat mid-turn has it queued until its turn ends; a chat that is asking the person something,
  has reported no state, or has ended is sent nothing, and its next turn is handed the line
  instead. An Enter never lands on a harness's own prompt.

An Ask whose browser ends first (closed, ended by a narrowing, its chat closed, Stop all) ends
with it and wakes nothing: the Inbox keeps an Update, and the chat's next browser call says the
browser has gone. A fifth send is a new amendment here.
