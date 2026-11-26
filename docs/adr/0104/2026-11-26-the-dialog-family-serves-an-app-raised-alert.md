# Amendment, 2026-11-26: the dialog family serves an alert the app raises

Amends [ADR 0104](../0104-the-window-is-built-from-component-families-on-react-aria-each-primitive-wrapped-once.md)
and its [dialog family](2026-10-22-the-dialog-family-and-the-top-layer.md), by the person's
decisions recorded in
[ADR 0130](../0130-prerequisites-what-a-project-needs-from-the-person-before-its-chats-can-work.md).

**An alert the app raises is the dialog family's `AlertDialog`** (`app/src/ui/dialog/Dialog.tsx`),
as one a press opens is. Its first is an open **prerequisite**'s modal, opened from the core's
"prerequisites changed" event with no press of the person's. No screen draws one by hand, and the
family's rules hold whole: three rows with the footer in view, at most three answers in fixed
places, ⏎ and Esc, no dialog over another.

**What an app-raised alert adds:**

- **Its answers' places.** The answer that grants nothing (for a prerequisite, Work in strict mode
  or Keep my current rules) is the `cancel` slot: it leads, it is Esc's, and it has focus first,
  because the person did not ask for the dialog and may be typing when it opens. The granting
  answer (Accept) is the `main` slot, of the `grants` variant, so Return never presses it, from a
  field or otherwise (`returnFinds`).
- **A settle guard.** No key and no press reaches an answer for a settle interval after the alert is
  drawn, or after its content changes (`useNoticeSettle`, #1695). A key held down while it opens
  answers nothing.
- **No silent swap.** If what it shows changes while it is open (a new digest, a new reason), it
  says so, draws the new content, arms the settle guard again, and moves focus off Accept to the
  answer that grants nothing.
- **When it may open.** Never during an IME composition or while a key is down, and never over an
  open menu, select or command palette, as never over another dialog: it waits for them to close.
- **One at a time, one per window.** Several waiting alerts are shown one after another, in the
  order the core gives, each saying where it stands ("1 of 2").
- **A press in flight** marks the alert `aria-busy` until its answer lands.
- **What a screen reader gets.** `aria-describedby` names the why-line and the summary's count; the
  summary is a real list; a body that scrolls is focusable, so the keyboard can scroll it.
- **Focus returns** to where it was when the alert closes.
- **It closes where it is answered.** An answer in one window closes the same alert in every window,
  through the event that raised it.
