### Changed

- **A clone made with `purlis clone` says a chat's gh gets no Agent identity token for it.** Only
  the window records a clone for the GitHub App mint, so `purlis clone` now says so and names
  the way out (re-clone from the window, or press *Trust this clone* there). A chat starting in a
  workspace with such a clone is told at its start, and a `gh` run inside one is refused
  naming the clone, where it used to be handed a token that left that repo out (#7196).
