# Amended for #2967, the first key reads the tree around its field, 2026-10-21

1. **The first field is blanked from the tree around it.** Before the person's first key into a
   field of a page where they have typed into no field yet, the element that has the page's own
   document's focus (asked in purlis's world) is blanked from the tree around it alone
   (`Accessibility.getPartialAXTree` with its relatives: the element, the nodes around it up to
   the root, its siblings and its children), not from every frame's whole tree. It is used only
   where that tree marks the element focused and the element holds no frame; anything else
   reads the whole tree as before. Every later field of the page is found in the whole tree, so
   a code typed one character a box is kept out whole (#2144). *D-2967-1, decided in
   implementation, to be confirmed.*
2. **The tree is readied as the person takes over.** The engine makes its accessibility tree the
   first time it is asked, which on a large page is what a first key would wait for: as the
   person takes the browser over, its root is asked for once on a thread of its own, nothing of
   the page's world run and nothing kept. *D-2967-2, decided in implementation, to be
   confirmed.*
