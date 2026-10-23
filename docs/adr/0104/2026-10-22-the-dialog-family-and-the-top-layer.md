# Amendment, 2026-10-22 (#2910): the dialog family, and the window's top layer

**Every dialog and alert dialog is the dialog family's** (`app/src/ui/dialog/Dialog.tsx`), on
React Aria's `ModalOverlay`, `Modal` and `Dialog`; Radix's `Dialog` and `AlertDialog` are gone
from the window. `purlis/dialog-family` is an error: no screen writes `role="dialog"` or
`role="alertdialog"` itself. The family holds the dialog rules (`docs/ui-primitives.md`, the
dialog family): three rows with the footer always in view, a width by size, a height up to the
window less 48 px, a fixed top, at most three answers in fixed places, ⏎ and Esc, no dialog over
another, and where the keyboard starts.

**What another family opens inside a dialog is drawn in the window's top layer**
(`app/src/overlays/topLayer.ts`). A React Aria modal makes everything outside it unreachable and
holds the keyboard in itself; a select's list or a menu that a control inside the dialog opens is
drawn in a portal outside it. The families still on Radix draw their portals into one element
marked as React Aria's top layer, which it keeps reachable and lets the keyboard into, stacked over
the window's rows and its dialogs. A family that moves onto React Aria draws its overlays as React
Aria's own, which know the modal they are in.
