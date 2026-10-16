# Amended for #2303, screenshots paint over what the page draws, 2026-10-17

Spec #2008 rules that what the person typed never reaches the model, in a snapshot or a
screenshot (the operator's ruling of 2026-10-14). The amendment for #2014, item 4, has a
screenshot paint over every field the person typed into and every element whose text, as the
page's tree reads it, shows what they typed. This note widens that item to everything the page
draws; nothing else changes, and the session protocol does not.

1. **A screenshot reads what the page lays out, not only its tree.** Once a page keeps anything
   out (a field purlis filled, or what the person typed), each screenshot of it reads the
   engine's own layout of every document on the page, each frame read in its own session, before
   and after the picture, with no script in the page. It paints over every text laid out that
   holds a text kept out, whatever the tree says of it (hidden from the tree, drawn by the page's
   style, deeper than a snapshot reads), and every field, button or drop-down whose value or
   options hold one. *D-2303-1, to be confirmed.*
2. **What cannot be read is painted whatever it shows.** On such a page every canvas, image,
   video and embedded object, every element drawn with an image or a paint script, and every
   frame whose document no read holds are painted over, whether or not they show a text kept
   out. *D-2303-2, to be confirmed.*
3. **Unsure means withheld.** A screenshot whose page cannot say what it lays out, or that draws
   an element again outside its box, is withheld in words. *D-2303-3, to be confirmed.*
