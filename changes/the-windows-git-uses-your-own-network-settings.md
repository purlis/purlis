### Changed

- **The window's git uses only your own network settings, not a repository's.** A fetch, push
  or save the window runs takes its proxy, TLS and certificate settings, extra headers and
  address rewrites from your global or system git config only. Where a repository's own config
  sets one of these, the window's git uses your value or git's default instead, or stops with a
  sentence naming the kind of setting and asks you to move it to your global config.
