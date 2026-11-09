### Fixed

- **A theme that fills a tree or a table is held to what stands in its rows.** A tree's or a
  table's row draws no fill of its own, so the parts check never measured what is drawn in it on a
  fill a theme gave it. A badge, a count or a field's error in such a row is now read on that fill,
  and a fill they can't be read on draws the row with no fill instead (#5863).
