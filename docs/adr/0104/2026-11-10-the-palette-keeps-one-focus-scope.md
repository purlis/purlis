# Amendment, 2026-11-10 (#6093): the palette keeps one focus scope

**The palette (`DialogSurface`) holds the keyboard with the one focus scope React Aria's
`Overlay` draws round it**, not a `FocusScope` (`contain`) of its own as the amendment of
2026-11-04 put it. `NotModal` drew its own inside the `Overlay`'s, and the `Overlay`'s already
contains: React Aria's dialog asks it to (`useDialog` calls `useOverlayFocusContain`), so Tab and
Shift+Tab went round the palette with either scope alone. #6090 found the second one; #6093
removed it.

**What holds it.** `Dialog.test.tsx`'s "keeps the keyboard in itself on Tab and Shift+Tab, round
from either end" presses each Tab with `pressTab` (`ui/dialog/dialog.testkit.ts`), which fails
unless a focus scope took it, and counts one scope. With the `Overlay`'s scope turned off
(`disableFocusManagement`) and nothing else, its first press is left to the engine and it fails.

Everything else of 2026-11-04 stands: the palette is not modal, and Esc and a press outside it
close it.
