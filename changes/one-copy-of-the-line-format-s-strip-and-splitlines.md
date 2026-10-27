### Changed

- **`purlis memory` snippets read a memory's frontmatter the way every other reader does.**
  The line shown under a search hit is the first line worth showing below the frontmatter's
  closing `---` line. A `---` with text after it, or indented, is text and closes nothing.
  A memory whose frontmatter is never closed is read as all body. Every part of purlis now
  strips and splits a line through one copy of Python's `str.strip` and `str.splitlines`
  (#3482).
