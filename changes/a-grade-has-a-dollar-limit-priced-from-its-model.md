### Changed

- **A grade's dollar limit is priced from the model its chats run on** (Skill studio preview,
  #3469). The plan says the limit and the price it used: the model's, read from the profile's
  `--model`, `ANTHROPIC_MODEL` or your Claude Code settings, or the dearest known price where
  none is named or its price isn't known, so the limit is never set too low. A grade that reaches
  the limit stops, and its record says it stopped at its dollar limit. A chat that says its tokens
  and not its cost counts against the limit at the dearest price. Prices come from a table in
  purlis, never from the network.
