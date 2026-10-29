### Changed

- **A workspace's notices stack on its chip, and its clone shows there** (#3649). Several
  notices about one workspace now share one card on its chip, newest first, with a line that
  says how many. The chip's dot shows the most urgent of them: ● for news, ▲ for a warning, a
  shape and not a colour alone, and the chip's description says the count and the most urgent
  kind. Cloning a new workspace's repos now shows on its chip as "Cloning N repos…", counting as
  each repo lands, and turns in place into "Cloned …" or "Could not clone …", which offers Retry
  and the workspace's repo settings. It never closes on a timer.
