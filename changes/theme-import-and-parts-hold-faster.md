### Changed

- **Importing a theme and holding a parts theme's floors take less time.** The consent floors an
  import's walk holds are measured once per theme rather than once per point that draws the same
  one, and a parts theme's consent looks are drawn once for every workspace tint rather than once
  a tint. Every theme draws exactly as before (#3597).
