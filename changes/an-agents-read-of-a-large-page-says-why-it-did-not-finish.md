### Fixed

- **A chat's read of a large page finishes in time, or says why.** A snapshot, find or wait on
  a very large page that the browser takes long to read answers inside the chat's call limit:
  with the page, or saying in words that the page is too large to read in time, never that
  whether it acted is not known. The read goes on, so the same call made again in a moment gets
  the page. Handing a browser back after typing into such a page no longer outlasts the window's
  wait, and a code you typed one character a box stays out of the chat's answers even where the
  page could not be read again (#3277).
