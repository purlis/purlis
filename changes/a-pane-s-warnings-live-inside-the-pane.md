### Changed

- **A chat's warnings live inside its pane, and the chosen tab stays joined to it** (#3142).
  A pane is three rows now: a band of its warnings at the top, each a washed card with its
  words above its buttons, two at most and the rest behind "+N more"; a slim header with the
  chat's skills, its harness and its context gauge at the left and the pane's split and close
  at the right; then the terminal. Nothing sits between a tab and its pane any more, and
  nothing is drawn over the terminal's first lines. The terminal keeps its last line in place
  as a warning comes or goes. Esc on a warning puts it away and moves on to the next one, or
  to the terminal; a question about access (Allow or Keep blocked) has no Dismiss and stays.
