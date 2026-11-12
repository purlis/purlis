### Fixed

- **An artifact page hears the answers to what it asks as it opens.** A page that called its
  queries the moment it loaded, as after its script was approved, could hear nothing back: the
  window took the page's load as starting only once the webview said so, which can come after
  the page has already asked, and dropped those asks. A load now starts when the page's document
  is asked for, before the page can ask anything (#6232).
