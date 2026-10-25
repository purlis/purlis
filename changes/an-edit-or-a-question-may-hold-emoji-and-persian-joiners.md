### Fixed

- **An edit to a file with a family emoji or Persian text can be allowed from the window.** A
  change holding two emoji joined by a zero-width joiner, or a Persian word written with a
  zero-width non-joiner between two letters, was never shown whole, so its Allow was missing
  and it could only be answered in the pane. It is now shown and may be allowed. A joiner
  anywhere else, such as between two Latin letters, still keeps the change from being shown.
- **A question's labels keep a Persian word's non-joiner.** A form an agent asks you to fill
  dropped the zero-width non-joiner from Persian labels and text, which ran the word's parts
  together. It is kept now, and so is a family emoji; a joiner anywhere else is still dropped.
