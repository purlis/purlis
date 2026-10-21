# Amended for #2967, the first key reads the tree around its field, 2026-10-21

1. **The first field is blanked from the tree around it.** Before the person's first key into a
   field of a page where they have typed into no field yet, the element that has the page's own
   document's focus (asked in purlis's world) is blanked from the tree around it alone
   (`Accessibility.getPartialAXTree` with its relatives: the element, the nodes around it up to
   the root, its siblings and its children), not from every frame's whole tree. It is used only
   where that tree marks the element focused and the element is a field a person types into (a
   text box, search box, combo box or spin button by its role; an input, a text area or an
   editable element by its tag) that holds no document of its own; anything else reads the
   whole tree as before. Every later field of the page is found in the whole tree, so a code
   typed one character a box is kept out whole (#2144). *D-2967-1, decided in implementation, to
   be confirmed.*
2. **What the first key takes, measured.** On the large page of #2295 (20,000 rows, about
   60,000 nodes), on macOS real Chrome: the first key took 23 to 46 s when it read every frame's
   whole tree, and takes 250 to 450 ms from the tree around its field; every later key into the
   field takes a few milliseconds. Enabling the engine's accessibility domain as the person
   takes over, to have the tree made before the first key, was measured and did not bring the
   first key under the spec's 50 ms (448 ms a second after the take-over), so purlis does not do
   it. The first key on such a page stays over the bound; what is left of it is the engine's
   own making of its tree for the page.
