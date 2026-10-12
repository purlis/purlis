### Added

- **A terminal editor opens in a shell tab, at the line.** With `$VISUAL` or `$EDITOR` chosen as
  your editor, *Open in your editor* used to start vi, vim, nvim, nano, `emacs -nw` and the like
  with no terminal, so they exited at once. purlis now opens an editor it knows to need a
  terminal in a shell tab of its own, in front, standing in the file's folder, with `+line` and
  the file. The editor and its words are handed to the tab as they are, never typed into a
  shell, so nothing in the variable or in the file's name is read as a command. An editor that
  opens its own window, such as gvim or `emacsclient -c`, is started as before (#1042).
