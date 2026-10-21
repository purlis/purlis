# Amended for #2972, the field blanked has the document's focus, 2026-10-21

1. **The tree's focused element is confirmed by its document.** Before the person's key that
   edits, the element the tree marks focused is blanked only where its document agrees. Where
   its document says another element has its focus (a field naming an active option,
   `aria-activedescendant`), that element, which the key goes to, is blanked and read after the
   key instead; where the document cannot say, the tree's element is, as before. Where the tree's
   element is inside the one the document names, its shadow roots included (a field in a shadow
   root, open or closed, whose document names its host), the tree's element stays: the keys go
   to it, and its value is read from it. One read of the
   document's focus, in purlis's world, no read of the tree. *D-2972-1, decided in
   implementation, to be confirmed.*
