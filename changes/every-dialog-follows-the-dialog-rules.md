### Changed

- **Every dialog follows one set of rules.** A dialog's title stays at its top and its answers
  stay in view at its bottom, under a rule, while only what is between them scrolls, so no
  dialog scrolls its buttons out of reach. A dialog is as wide as its size, never its content, at
  most the window less 48 px tall, and anchored 24 px under the window's top, so a change in what
  it holds moves only its bottom edge; a button whose words change ("Create", "Creating…") keeps
  its width. Cancel always leads and the main action always trails, in the same place in every
  dialog, with at most one other answer between them; Return in a field is the main action,
  except for an act that cannot be taken back or one that grants, and Escape closes one layer at
  a time. An alert dialog starts on its safe answer, and no dialog or confirm row starts on an
  answer that grants, so a key pressed by habit never answers yes (#2980). A list of more than seven items in a dialog has a filter (Open vault, Add to a
  chat's context, Extensions). Reviewing an extension asks in place of the Extensions dialog
  rather than over it. A theme can set a dialog's surface, title and footer rule through its
  parts (`dialog.root.corner-radius`, `dialog.title.font-weight`), and `purlis theme parts` lists
  them; a dialog's corner is drawn at most 16 px at the default text size, and its quieter lines
  are held to 4.5:1 in the tier they are drawn in (#2910).
