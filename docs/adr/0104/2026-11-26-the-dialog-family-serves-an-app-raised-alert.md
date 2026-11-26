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

- **One at a time.** Several waiting alerts are shown one after another, in the order the core
  gives. Where another dialog is up, the alert waits for it to close, since no dialog opens over
  another.
- **Where the keyboard starts.** Focus starts on the answer that grants nothing (for a
  prerequisite, Work in strict mode), because the person did not ask for the dialog and may be
  typing when it opens. That answer is also Esc's.
- **A grant is pressed, never found.** The granting answer (Accept) is of the `grants` variant, so
  Return never presses it, from a field or otherwise (`returnFinds`).
- **It closes where it is answered.** An answer in one window closes the same alert in every window,
  through the event that raised it.
