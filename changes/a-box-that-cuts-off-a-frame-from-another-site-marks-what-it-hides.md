### Fixed

- **A page that holds a frame from another site settles.** A chat's browser waited out its whole
  settling time on such a page and then said the frame's document was still loading, because the
  engine answers that document in the frame's own session. It now settles once the frame has
  loaded (#2689).
- **A box that cuts off a frame from another site marks what it hides.** A snapshot or a find
  marks a frame's element `(out of view)` when a box on the page around the frame hides that
  part of it, for a frame from another site as for one from the page's own (#2689).
