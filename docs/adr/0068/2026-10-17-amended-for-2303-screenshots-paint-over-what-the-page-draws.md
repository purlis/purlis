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
   style, deeper than a snapshot reads, split over elements or spaced otherwise), and every
   field, button or drop-down whose value, options or labels hold one, on every site; and every
   frame whose document no read holds. *D-2303-1, to be confirmed.*
2. **What cannot be read is painted whatever it shows, on the site the text was kept on.** Once a
   text is kept out on a page, every canvas, image, video and embedded object, and every element
   drawn with an image or a paint script, is painted over, whether or not it shows a text kept
   out, on every page whose top-level site is that page's: its registrable domain by the public
   suffix list, or for an IP address or `localhost` its host and port. A frame or a page opened
   from such a page counts by that page's site, and a page whose site cannot be told is taken as
   its opener's, or as kept when it has none. On other sites, kept text is still painted over
   wherever it is drawn. This is the person's ruling of 2026-10-17. *D-2303-2, ruled.*
3. **Unsure means withheld.** A screenshot is withheld in words when its page cannot say what it
   lays out, or draws a reflection. *D-2303-3, to be confirmed.*
4. **What moves is painted the whole way.** Each item painted over is placed before and after the
   picture, and the box from where it was to where it is is painted, so an item that moves while
   the picture is taken is covered wherever the picture caught it. *D-2303-9, to be confirmed.*

The match is literal: a page that transforms or redraws what it shows is outside what a
screenshot can check.
