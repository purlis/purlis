### Changed

- **`purlis memory` snippets read a memory's frontmatter the way the persona and skill
  readers do.** The line shown under a search hit is the first line worth showing below the
  frontmatter's closing `---` line. A `---` with text after it, or indented, is text and
  closes nothing. A memory whose frontmatter is never closed shows no line. purlis-core's
  memory, inventory, workspace-section, git-config and init readers now strip and split lines
  through the line format's one copy of Python's `str.strip` and `str.splitlines` (#3482).
