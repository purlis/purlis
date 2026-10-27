### Changed

- **The window marks the characters you cannot see the way the command line does.** A
  sandbox block's folder or host, a recent grant, a runner's branch and a repository's
  instruction file now show each invisible character as `⟦U+XXXX⟧`, where they used to show
  `‮` or `U+202E`. They mark the same characters as the core, from the same generated
  table: a variation selector, a Hangul filler or a no-break space in an instruction file is
  marked in its preview too, and a joiner that holds an emoji or a word together still reads as
  text. A command holding a no-break or an en space now shows it as `⟦U+00A0⟧` in the Inbox,
  where it used to read as a plain space. The Inbox also says that this is why only Deny is
  offered (#3202).
