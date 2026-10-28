### Added

- **A chat's browser downloads and uploads files, inside fences.** A download lands in the
  browser's own holding folder, which no chat reaches, under a name the browser makes, never
  the page's. The agent is told each download's name, size and origin, and a download is
  cancelled past 512 MB, or past 1 GB for a browser's downloads together. Saving one into the
  chat's folder needs your yes for its origin, which purlis does not ask for yet, so nothing is
  saved for now. An upload takes files from the chat's own folder alone, under its read rules,
  following no link out of it; the browser is handed purlis's copy, never the chat's file
  (#2020).
