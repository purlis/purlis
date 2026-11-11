### Fixed

- **The extension record opens in a shell tab for a terminal editor.** _Open in your editor_ in
  Extensions starts a terminal `$VISUAL` or `$EDITOR`, such as vim or nano, in a shell tab of the
  project in front, as a harness declaration's _Open file_ does, instead of starting it with no
  terminal so that nothing showed. With no project open, the dialog says why it cannot open
  there and what to do instead.
