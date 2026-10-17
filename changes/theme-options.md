### Added

- **Theme options.** A theme folder's `theme.json` can declare toggles and choices, each value a
  layer of token values over every appearance (and over one appearance by its id). An option
  nothing chose draws its default. `purlis theme check` covers every combination of appearance
  and option values and names the combination each warning is found in; a theme with more than
  64 combinations is refused with its count. Settings › Appearance shows the options of the
  theme drawn, with where each value comes from (#2325).
