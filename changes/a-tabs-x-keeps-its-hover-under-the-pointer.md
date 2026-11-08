### Fixed

- **A theme's colours for a tab's × no longer cover its look under the pointer.** A theme
  that set `tabs.closer.text` or `tabs.closer.fill` drew that colour on the × even under the
  pointer, over the quiet hover, where the theme check never read it. The × now keeps its hover
  there, and the check reads its words on it over the tab's own fill, so a theme whose tab fill
  hides them draws that tab's default (#3269).
