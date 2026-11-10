# Amendment, 2026-11-10: keyboard triage, and the Asks keyboard ruling

Built for #6016 from [ADR 0121](../0121-attention-what-waits-on-you-reaches-you-and-nothing-else-interrupts.md)
decision 9, which takes over #2812's J/K keys and #3123's keyboard ruling. The particulars below
are what that decision left open. They are built in `app/src/asksTriage.ts` and the Asks list
(`app/src/Inbox.tsx`).

## The ruling: grid rows

#3123 asked for one of two keyboards for Asks: one up/down sequence through the asks and their
buttons (the Radix roving focus it had then), or grid rows. **Grid rows**, as the steward recorded
under the operator's standing order of 2026-11-08 and as Asks has been drawn since it moved onto
the list family: ↑ and ↓ move between rows, ← and → move through a row's own buttons, Home and
End go to the first and last row, typing finds no row, and a reply box keeps its own arrows. This
is the WAI-ARIA grid pattern and the list family's (React Aria GridList), so Asks walks as every
other list of rows with buttons does. One flat sequence would have needed a mode of its own in
the family. The operator can reverse this.

## Where the keyboard lands

- **⌘⇧I begins triage**, from wherever the keyboard is. Pressed again in Asks, it keeps where
  triage began.
- **It lands on the first ask's lead.** That is its **Answer** where it is answered behind one.
  Otherwise it is the answer its source suggests, or else its first answer that grants nothing
  (Deny, Keep blocked). It is never an Allow: **a grant never takes the keyboard as it is drawn**
  (#2980), and a row the keyboard is moved onto is one drawn to it. The Allow is one → away and
  is pressed on purpose. A row answered elsewhere (a chat in another window, another project of
  this window) leads with its Go to chat or Go to project.
- **Answering moves the keyboard on**: to the lead of the ask that stood after the one answered,
  and to the first again past the last. The ask after can be another project's. Its row's
  **Go to project** brings that project forward, and triage goes on in its Asks, at its first
  ask's lead.
- **The settle guard holds for every press** (ADR 0097, #1695). The row the keyboard moves onto
  has just moved, so an Allow pressed on its heels is refused and says why, as a click is.
- **A high-risk ask is never moved onto** (decision 8). Its row is marked, and triage passes it
  by: first, next and later alike. It is answered from the row the person moves to with the
  arrows. Which asks are high-risk is #6011's to say. Until then none is (`highRisk`, the seam).

## The keys

On an ask's row, each key presses the row's own button, so a key is refused wherever a click
would be. They follow the finder's keys on its aimed row: ⌘⌥ and a letter on a Mac, Alt+Shift and
the letter elsewhere, read by the key's place. **No key of a single letter does anything** (I-11):
J and K are not taken up, so a stray keystroke never acts on an ask. None acts from a reply box.

| Key | Act |
| --- | --- |
| ⌘⌥L (Alt+Shift+L) | **Later**: on to the next ask's lead. The ask stays, and comes round again. |
| ⌘⌥G (Alt+Shift+G) | **Go to chat**, which ends triage there. On another project's row, Go to project. |
| ⌘⌥S (Alt+Shift+S) | **Snooze** its chat, until it asks something new. |
| ⌘⌥X (Alt+Shift+X) | **Stop** its chat. |
| Enter | Presses the button the keyboard is on: the answer, under the settle guard. |
| Esc | Leaves triage. |

They are in the key registry (`asks.*`) and so on the cheat sheet (⌘/).

## Where the keyboard goes back to

When Asks has nothing left that triage can land on, or on Esc, the keyboard goes back to where
it was before ⌘⇧I. If that element has gone, it goes to the chat in front, as Esc always did.
Moving the keyboard out of Asks by any other way (a click, Tab, Go to chat) ends triage there.
Nothing new is stored, so no tier is named (ADR 0069).
