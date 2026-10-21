### Fixed

- **A browser you take over after its chat clicked a field takes your keys.** Your first key was
  held with "the page cannot tell which field has the focus", since the page said that both
  the document and the field had the focus. The field is what has it (#2294).
