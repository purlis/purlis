# Amended again for #2303, what cannot be read and its shadow, 2026-11-10

On a page whose site a text was kept out on, a screenshot paints over every canvas, image,
video, embedded object and element drawn with an image or a paint script, since what they show
cannot be read. This note adds how far that paint reaches; nothing else changes.

1. **What cannot be read is painted with its shadow.** Each such item is widened by the
   `drop-shadow(…)` filters it is drawn through, its own and those of every element around it,
   one drawn through the next, as a painted text and a blanked field already are.
2. **A filter whose reach cannot be read withholds the screenshot.** Where such an item is drawn
   through a filter of the page's own (`url(…)`) or a shadow in lengths other than pixels, the
   read flags it, and a screenshot that paints what cannot be read is withheld.
3. **Elsewhere the flag changes nothing.** On a page whose canvases and images are not painted,
   their filters neither widen anything nor withhold the screenshot.
4. **A text's shadow is not read for them.** `text-shadow` draws an element's text, not what an
   image or a canvas shows; that text is read as any other.
