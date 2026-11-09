# Amendment, 2026-11-09 (#2946): a theme option can carry component parts

§2 gave a theme one `parts` object, beside its tokens, for every appearance. A theme option
(a toggle or a choice, each value a layer of tokens) could change colours, but not a shape: a
"round buttons" toggle or a density look could not be offered.

- **An option's value is a layer of parts too.** A toggle's `on` and a choice's value may hold
  `parts`, with the theme's keys, values and bounds, read by the same reader. Option layers are
  for every appearance; there is no per-appearance `parts`, as the theme has none.
- **Laid over the theme's in the options' order**, key by key, as a theme's are over its
  parent's: a later option's key wins.
- **Held as the theme's are.** A key purlis does not have is said and left out, a value outside
  its bound refuses the theme, each naming the option. Sizes are held in both densities in every
  combination of option values that sets parts. The floors and the consent rules hold in every
  combination the check measures, since each combination is drawn with its parts before it is
  measured.
- **The schema's option layer names the theme's `parts` block** (`#/properties/parts`), so an
  editor completes an option's keys from the same manifest. A choice's value gains an optional
  key in place inside the option's `oneOf`, whose branches stay told apart by `kind`; the schema's
  only-grows rule now compares such a `oneOf` branch by branch.
