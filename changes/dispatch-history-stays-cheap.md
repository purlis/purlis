### Changed

- A project that has run thousands of tasks no longer reads every past dispatch to draw the
  sidebar's task status or each turn's list of open tasks. Ended dispatches are summarised in
  one small file in the dispatch store (`app/dispatches/ended.json`), and only running records
  are read in full. Deleting the file is safe: the next read rebuilds it (#5502).
